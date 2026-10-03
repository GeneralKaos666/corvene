package com.wasimaster.corvene;

import android.app.NativeActivity;
import android.content.ActivityNotFoundException;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.webkit.MimeTypeMap;
import java.io.FileInputStream;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.provider.DocumentsContract;
import android.provider.Settings;
import android.text.InputType;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.Toast;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * Corvene's one activity. The application is native code: NativeActivity
 * loads libcorvene.so and hands it the window, the input queue and the
 * lifecycle. What a NativeActivity does not get is text: the input method
 * only talks to a view with an InputConnection, so an invisible focused view
 * provides one and passes what the keyboard does to the native side
 * (crates/corvene/src/android.rs).
 */
public class CorveneActivity extends NativeActivity {
    static {
        // NativeActivity opens the library with dlopen, which does not make
        // its JNI functions known to this class loader.
        System.loadLibrary("corvene");
    }

    private static CorveneActivity instance;
    private InputView inputView;

    @Override
    protected void attachBaseContext(Context base) {
        super.attachBaseContext(base);
        GrammarModule.attach(this);
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // a background fetch WorkManager started in this process without an
        // activity (CorveneFetchWorker) has to let go before the native
        // side starts
        nativeEndHeadless();
        super.onCreate(savedInstanceState);
        instance = this;
        // NativeActivity leaves the keyboard's initial state to the system,
        // which opens it for a focused editor on some devices; it should
        // only come up when a text field is focused (showKeyboard).
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_HIDDEN
                | WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        // What the system bars, a cutout and the keyboard cover. Edge to
        // edge (enforced when targeting Android 16) the window is not
        // resized for them, so the native side is told.
        getWindow().getDecorView().setOnApplyWindowInsetsListener((view, insets) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                int types = android.view.WindowInsets.Type.systemBars()
                        | android.view.WindowInsets.Type.displayCutout();
                // The keyboard too: full screen (and edge to edge) the
                // window is not resized above it. Where it still is, the
                // native side takes the larger of this and the resize.
                types |= android.view.WindowInsets.Type.ime();
                android.graphics.Insets covered = insets.getInsets(types);
                nativeInsets(covered.left, covered.top, covered.right, covered.bottom);
            } else {
                nativeInsets(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                        insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            }
            return view.onApplyWindowInsets(insets);
        });
        // the window reaches under a cutout on every edge; the native side
        // keeps its content clear of it (nativeInsets)
        if (Build.VERSION.SDK_INT >= 28) {
            WindowManager.LayoutParams attributes = getWindow().getAttributes();
            attributes.layoutInDisplayCutoutMode = Build.VERSION.SDK_INT >= 30
                    ? WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
                    : WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
            getWindow().setAttributes(attributes);
        }
        applyFullScreen();
        acceptDrops();
        inputView = new InputView(this);
        addContentView(inputView, new ViewGroup.LayoutParams(1, 1));
        inputView.requestFocus();
        // a link is read by the platform (`getIntent().getDataString()`)
        if (ACTION_NOTIFICATION.equals(getIntent().getAction())
                || Intent.ACTION_SEND.equals(getIntent().getAction())) {
            handleIntent(getIntent());
        }
        CorveneFetchWorker.schedule(this);
    }

    /** Whether the native application runs in this process. */
    static boolean isRunning() {
        return instance != null;
    }

    /**
     * Back from a picker, a chooser or another application: the input view
     * takes the focus again, or hardware keys go nowhere until a tap.
     */
    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus && inputView != null) {
            inputView.requestFocus();
        }
        if (hasFocus) {
            // the system shows its bars again for a dialog or the keyboard
            applyFullScreen();
        }
    }

    private static final String FULL_SCREEN = "full-screen";

    private boolean isFullScreen() {
        return getSharedPreferences(PREFERENCES, MODE_PRIVATE).getBoolean(FULL_SCREEN, true);
    }

    /**
     * Full screen (the default, View > Toggle full screen turns it off): the
     * status and navigation bars are hidden and a swipe from an edge shows
     * them for a moment, as in a game. A phone's screen is small for a
     * window made for the desktop.
     */
    @SuppressWarnings("deprecation")
    private void applyFullScreen() {
        boolean fullScreen = isFullScreen();
        View decor = getWindow().getDecorView();
        if (Build.VERSION.SDK_INT >= 30) {
            android.view.WindowInsetsController controller = decor.getWindowInsetsController();
            if (controller == null) {
                return;
            }
            int bars = android.view.WindowInsets.Type.systemBars();
            if (fullScreen) {
                controller.setSystemBarsBehavior(
                        android.view.WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                controller.hide(bars);
            } else {
                controller.show(bars);
            }
        } else {
            decor.setSystemUiVisibility(fullScreen
                    ? View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                    : View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
        }
    }

    /**
     * Starts the application again in a new process (a flag that needs a
     * relaunch changed): the task is restarted, then this process ends.
     */
    static void relaunch() {
        CorveneActivity activity = instance;
        if (activity == null) {
            return;
        }
        activity.runOnUiThread(() -> {
            Intent launch = activity.getPackageManager()
                    .getLaunchIntentForPackage(activity.getPackageName());
            if (launch == null || launch.getComponent() == null) {
                return;
            }
            // a moment for the settings just changed to reach the disk
            new android.os.Handler(android.os.Looper.getMainLooper()).postDelayed(() -> {
                activity.startActivity(Intent.makeRestartActivityTask(launch.getComponent()));
                Runtime.getRuntime().exit(0);
            }, 300);
        });
    }

    /**
     * Back to the activity from a browser tab opened over it (the sign-in's
     * callback arrived on the loopback listener): the activity is the root
     * of its task, so starting it again closes what is above it.
     */
    static void bringToFront() {
        CorveneActivity activity = instance;
        if (activity == null) {
            return;
        }
        activity.runOnUiThread(() -> {
            try {
                activity.startActivity(new Intent(activity, CorveneActivity.class)
                        .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP
                                | Intent.FLAG_ACTIVITY_SINGLE_TOP));
            } catch (RuntimeException e) {
                // the page says to return to Corvene
            }
        });
    }

    /** A short message over whatever is on screen. */
    public static String toast(final String message) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return "no activity";
        }
        activity.runOnUiThread(
                () -> Toast.makeText(activity, message, Toast.LENGTH_LONG).show());
        return null;
    }

    /** View > Toggle full screen. */
    static void toggleFullScreen() {
        CorveneActivity activity = instance;
        if (activity == null) {
            return;
        }
        activity.runOnUiThread(() -> {
            activity.getSharedPreferences(PREFERENCES, MODE_PRIVATE).edit()
                    .putBoolean(FULL_SCREEN, !activity.isFullScreen()).apply();
            activity.applyFullScreen();
        });
    }

    @Override
    protected void onDestroy() {
        if (instance == this) {
            instance = null;
        }
        super.onDestroy();
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handleIntent(intent);
    }

    /** Called from the native thread: show or hide the on-screen keyboard. */
    public static void showKeyboard(final boolean show) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return;
        }
        activity.runOnUiThread(() -> {
            InputMethodManager manager =
                    (InputMethodManager) activity.getSystemService(Context.INPUT_METHOD_SERVICE);
            if (show) {
                activity.inputView.requestFocus();
                manager.showSoftInput(activity.inputView, 0);
            } else {
                manager.hideSoftInputFromWindow(activity.inputView.getWindowToken(), 0);
            }
        });
    }

    // ── folders ─────────────────────────────────────────────────────────────

    private static final int REQUEST_PICK_FOLDER = 1;
    private static final String EXTERNAL_STORAGE = "com.android.externalstorage.documents";

    /**
     * Called from the native thread: the system's folder picker. The answer
     * comes through nativePathPicked: a path Corvene can use directly, or an
     * error for the user.
     */
    public static void pickFolder() {
        final CorveneActivity activity = instance;
        if (activity == null) {
            nativePathPicked(null, null);
            return;
        }
        activity.runOnUiThread(() -> {
            try {
                activity.startActivityForResult(
                        new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE), REQUEST_PICK_FOLDER);
            } catch (RuntimeException e) {
                nativePathPicked(null, "No folder picker is available on this device.");
            }
        });
    }

    private static final int REQUEST_PICK_FILE = 4;
    /** The largest file the file picker copies (an SSH key is a few kB). */
    private static final long PICKED_FILE_LIMIT = 16L << 20;

    /**
     * Called from the native thread: the system's file picker. The picked
     * document is copied into the cache directory (a document has no path)
     * and that copy's path goes to nativePathPicked.
     */
    public static void pickFile() {
        final CorveneActivity activity = instance;
        if (activity == null) {
            nativePathPicked(null, null);
            return;
        }
        activity.runOnUiThread(() -> {
            try {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT)
                        .addCategory(Intent.CATEGORY_OPENABLE).setType("*/*");
                activity.startActivityForResult(intent, REQUEST_PICK_FILE);
            } catch (RuntimeException e) {
                nativePathPicked(null, "No file picker is available on this device.");
            }
        });
    }

    private void copyPickedFile(Uri document) {
        File folder = new File(new File(getCacheDir(), "tmp"), "picked");
        folder.mkdirs();
        File[] old = folder.listFiles();
        if (old != null) {
            for (File file : old) {
                file.delete();
            }
        }
        File copy = new File(folder, "file");
        try (java.io.InputStream in = getContentResolver().openInputStream(document);
                java.io.OutputStream out = new java.io.FileOutputStream(copy)) {
            if (in == null) {
                throw new java.io.IOException("the document cannot be read");
            }
            byte[] buffer = new byte[65536];
            long total = 0;
            for (int n; (n = in.read(buffer)) > 0;) {
                total += n;
                if (total > PICKED_FILE_LIMIT) {
                    throw new java.io.IOException("the file is too large");
                }
                out.write(buffer, 0, n);
            }
        } catch (java.io.IOException | RuntimeException e) {
            copy.delete();
            nativePathPicked(null, "Could not read the file: " + e.getMessage());
            return;
        }
        nativePathPicked(copy.getPath(), null);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == REQUEST_PICK_FILE) {
            final Uri document = resultCode == RESULT_OK && data != null ? data.getData() : null;
            if (document == null) {
                nativePathPicked(null, null);
            } else {
                new Thread(() -> copyPickedFile(document), "pick-file").start();
            }
            return;
        }
        if (requestCode != REQUEST_PICK_FOLDER) {
            return;
        }
        final Uri tree = resultCode == RESULT_OK && data != null ? data.getData() : null;
        if (tree == null) {
            nativePathPicked(null, null);
            return;
        }
        new Thread(() -> resolvePickedFolder(tree), "pick-folder").start();
    }

    /**
     * A picked folder as a path: one of Corvene's own (through its documents
     * provider), one on shared storage when "All files access" is granted,
     * else, for a Git repository, a copy imported into Corvene's storage.
     */
    private void resolvePickedFolder(Uri tree) {
        String documentId = DocumentsContract.getTreeDocumentId(tree);
        String authority = tree.getAuthority();
        if ((getPackageName() + ".documents").equals(authority)) {
            File folder = CorveneDocumentsProvider.fileOf(
                    CorveneDocumentsProvider.baseDirectory(this), documentId);
            nativePathPicked(folder != null ? folder.getPath() : null, null);
            return;
        }
        if (EXTERNAL_STORAGE.equals(authority) && documentId.startsWith("primary:")
                && hasAllFilesAccess()) {
            File folder = new File(Environment.getExternalStorageDirectory(),
                    documentId.substring("primary:".length()));
            nativePathPicked(folder.getPath(), null);
            return;
        }
        Uri root = DocumentsContract.buildDocumentUriUsingTree(tree, documentId);
        try {
            String name = displayName(root);
            if (!hasChild(tree, documentId, ".git")) {
                pickFailed(canRequestAllFilesAccess()
                        ? "Corvene can only use folders in its own storage. To use a folder "
                                + "on shared storage in place, allow \"All files access\" first."
                        : "Corvene can only use folders in its own storage. A folder that "
                                + "is a Git repository is imported (copied) when picked.");
                return;
            }
            File base = CorveneDocumentsProvider.baseDirectory(this);
            File target = new File(base, name);
            for (int n = 2; target.exists(); n++) {
                target = new File(base, name + "-" + n);
            }
            final String importing = getString(R.string.importing, name);
            runOnUiThread(() -> Toast.makeText(this, importing, Toast.LENGTH_LONG).show());
            copyTree(tree, documentId, target);
            nativePathPicked(target.getPath(), null);
        } catch (Exception e) {
            pickFailed("The folder could not be imported: " + e.getMessage());
        }
    }

    private void pickFailed(String message) {
        runOnUiThread(() -> Toast.makeText(this, message, Toast.LENGTH_LONG).show());
        nativePathPicked(null, message);
    }

    private String displayName(Uri document) {
        try (Cursor cursor = getContentResolver().query(document,
                new String[] {DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null,
                null)) {
            if (cursor != null && cursor.moveToFirst() && cursor.getString(0) != null) {
                return cursor.getString(0).replace('/', '_');
            }
        }
        return "repository";
    }

    private boolean hasChild(Uri tree, String documentId, String name) {
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, documentId);
        try (Cursor cursor = getContentResolver().query(children,
                new String[] {DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null,
                null)) {
            while (cursor != null && cursor.moveToNext()) {
                if (name.equals(cursor.getString(0))) {
                    return true;
                }
            }
        }
        return false;
    }

    private void copyTree(Uri tree, String documentId, File target) throws IOException {
        if (!target.mkdirs() && !target.isDirectory()) {
            throw new IOException("could not create " + target);
        }
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, documentId);
        String[] columns = {
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
        };
        try (Cursor cursor = getContentResolver().query(children, columns, null, null, null)) {
            while (cursor != null && cursor.moveToNext()) {
                String childId = cursor.getString(0);
                String name = cursor.getString(1);
                if (name == null || name.contains("/") || name.equals("..")) {
                    continue;
                }
                File child = new File(target, name);
                if (DocumentsContract.Document.MIME_TYPE_DIR.equals(cursor.getString(2))) {
                    copyTree(tree, childId, child);
                    continue;
                }
                Uri source = DocumentsContract.buildDocumentUriUsingTree(tree, childId);
                try (InputStream in = getContentResolver().openInputStream(source);
                        OutputStream out = new FileOutputStream(child)) {
                    if (in == null) {
                        throw new IOException("could not read " + name);
                    }
                    byte[] buffer = new byte[1 << 16];
                    for (int read; (read = in.read(buffer)) > 0; ) {
                        out.write(buffer, 0, read);
                    }
                }
            }
        }
    }

    /** MANAGE_EXTERNAL_STORAGE is granted (Android 11+). */
    public static boolean hasAllFilesAccess() {
        return Build.VERSION.SDK_INT >= 30 && Environment.isExternalStorageManager();
    }

    /** This build may run native code it downloaded (the foss flavour). */
    public static boolean allowsDownloadedCode() {
        return BuildConfig.DOWNLOADED_CODE;
    }

    // The tree-sitter grammars as Google Play's on-demand module (play
    // flavour; GrammarModule is a stub in foss).

    public static String grammarModuleDir() {
        final CorveneActivity activity = instance;
        return activity == null ? "" : GrammarModule.directory(activity);
    }

    public static void installGrammarModule() {
        final CorveneActivity activity = instance;
        if (activity != null) {
            activity.runOnUiThread(() -> GrammarModule.install(activity));
        }
    }

    public static void uninstallGrammarModule() {
        final CorveneActivity activity = instance;
        if (activity != null) {
            GrammarModule.uninstall(activity);
        }
    }

    /** 0: progress, 1: installed, 2: failed (with a message). */
    static native void nativeGrammarModule(int status, long received, long total, String error);

    /** This build declares MANAGE_EXTERNAL_STORAGE (the foss flavour). */
    public static boolean canRequestAllFilesAccess() {
        final CorveneActivity activity = instance;
        if (activity == null || Build.VERSION.SDK_INT < 30) {
            return false;
        }
        try {
            String[] permissions = activity.getPackageManager().getPackageInfo(
                    activity.getPackageName(), PackageManager.GET_PERMISSIONS)
                    .requestedPermissions;
            if (permissions != null) {
                for (String permission : permissions) {
                    if ("android.permission.MANAGE_EXTERNAL_STORAGE".equals(permission)) {
                        return true;
                    }
                }
            }
        } catch (PackageManager.NameNotFoundException e) {
            // our own package
        }
        return false;
    }

    /** The system's "All files access" page for Corvene. */
    public static void requestAllFilesAccess() {
        final CorveneActivity activity = instance;
        if (activity == null || Build.VERSION.SDK_INT < 30) {
            return;
        }
        activity.runOnUiThread(() -> {
            try {
                activity.startActivity(new Intent(
                        Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                        Uri.parse("package:" + activity.getPackageName())));
            } catch (RuntimeException e) {
                activity.startActivity(
                        new Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION));
            }
        });
    }

    // ── links and notifications ─────────────────────────────────────────────

    private static final String ACTION_NOTIFICATION = "com.wasimaster.corvene.NOTIFICATION";
    private static final String EXTRA_IDENTIFIER = "identifier";
    private static final String EXTRA_PAYLOAD = "payload";
    private static final String CHANNEL = "pull-requests";
    private static final String PREFERENCES = "corvene";
    private static final String ASKED_NOTIFICATIONS = "asked-notifications";
    private static final int REQUEST_NOTIFICATIONS = 2;

    /**
     * A page in a Custom Tab: the browser's tab drawn over Corvene, so
     * signing in does not leave the application. Custom Tabs are an intent
     * convention; a browser without them opens the page normally.
     */
    public static void openCustomTab(final String url) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return;
        }
        activity.runOnUiThread(() -> {
            Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
            Bundle extras = new Bundle();
            extras.putBinder("android.support.customtabs.extra.SESSION", null);
            intent.putExtras(extras);
            intent.putExtra("android.support.customtabs.extra.TOOLBAR_COLOR", 0xff24292e);
            try {
                activity.startActivity(intent);
            } catch (RuntimeException e) {
                Toast.makeText(activity, "No browser is installed.", Toast.LENGTH_LONG).show();
            }
        });
    }

    /** What the activity was opened with: a link or a tapped notification. */
    private void handleIntent(Intent intent) {
        if (intent == null) {
            return;
        }
        if (ACTION_NOTIFICATION.equals(intent.getAction())) {
            nativeNotificationClicked(intent.getStringExtra(EXTRA_IDENTIFIER),
                    intent.getStringExtra(EXTRA_PAYLOAD));
            return;
        }
        if (Intent.ACTION_SEND.equals(intent.getAction())) {
            openRepositoryAddress(intent.getStringExtra(Intent.EXTRA_TEXT));
            return;
        }
        String url = intent.getDataString();
        if (url != null) {
            nativeOpenUrl(url);
        }
    }

    /**
     * Text shared with or dropped on Corvene: when it holds a repository's
     * address (https, ssh or git@host:path) the native side opens it like an
     * x-corvene://openRepo link, which offers to clone it.
     */
    private static boolean openRepositoryAddress(String text) {
        if (text == null) {
            return false;
        }
        for (String word : text.trim().split("\\s+")) {
            if (word.matches("(?i)(https?|ssh|git)://[^/\\s]+/\\S+")
                    || word.matches("[\\w.-]+@[\\w.-]+:\\S+")) {
                nativeOpenUrl("x-corvene://openRepo/" + word);
                return true;
            }
        }
        return false;
    }

    /**
     * Drag and drop from another window (split screen, a desktop mode): a
     * dropped link or text with a repository's address opens it.
     */
    private void acceptDrops() {
        getWindow().getDecorView().setOnDragListener((view, event) -> {
            switch (event.getAction()) {
                case android.view.DragEvent.ACTION_DRAG_STARTED:
                    android.content.ClipDescription description = event.getClipDescription();
                    return description != null && (description.hasMimeType("text/*")
                            || description.hasMimeType(
                                    android.content.ClipDescription.MIMETYPE_TEXT_URILIST));
                case android.view.DragEvent.ACTION_DROP:
                    android.content.ClipData clip = event.getClipData();
                    for (int i = 0; clip != null && i < clip.getItemCount(); i++) {
                        CharSequence text = clip.getItemAt(i).coerceToText(this);
                        if (text != null && openRepositoryAddress(text.toString())) {
                            return true;
                        }
                    }
                    return false;
                default:
                    return true;
            }
        });
    }

    /** 0: not asked yet, 1: allowed, 2: denied. */
    public static int notificationPermission() {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return 2;
        }
        NotificationManager manager = activity.getSystemService(NotificationManager.class);
        if (manager.areNotificationsEnabled()) {
            return 1;
        }
        boolean asked = activity.getSharedPreferences(PREFERENCES, MODE_PRIVATE)
                .getBoolean(ASKED_NOTIFICATIONS, false);
        return Build.VERSION.SDK_INT >= 33 && !asked ? 0 : 2;
    }

    /** POST_NOTIFICATIONS, which Android 13 made a runtime permission. */
    public static void requestNotificationPermission() {
        final CorveneActivity activity = instance;
        if (activity == null || Build.VERSION.SDK_INT < 33) {
            return;
        }
        activity.runOnUiThread(() -> activity.requestPermissions(
                new String[] {"android.permission.POST_NOTIFICATIONS"}, REQUEST_NOTIFICATIONS));
    }

    /** The permission counts as asked for once the system's dialog is answered. */
    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions,
            int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        if (requestCode == REQUEST_NOTIFICATIONS) {
            getSharedPreferences(PREFERENCES, MODE_PRIVATE).edit()
                    .putBoolean(ASKED_NOTIFICATIONS, true).apply();
        }
    }

    public static void openNotificationSettings() {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return;
        }
        activity.runOnUiThread(() -> activity.startActivity(
                new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                        .putExtra(Settings.EXTRA_APP_PACKAGE, activity.getPackageName())));
    }

    /** Posts a notification; tapping it opens Corvene with its payload. */
    public static void showNotification(String identifier, String title, String body,
            String payload) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return;
        }
        NotificationManager manager = activity.getSystemService(NotificationManager.class);
        manager.createNotificationChannel(new NotificationChannel(CHANNEL,
                activity.getString(R.string.channel_pull_requests),
                NotificationManager.IMPORTANCE_DEFAULT));
        Intent intent = new Intent(activity, CorveneActivity.class)
                .setAction(ACTION_NOTIFICATION)
                .putExtra(EXTRA_IDENTIFIER, identifier)
                .putExtra(EXTRA_PAYLOAD, payload)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent open = PendingIntent.getActivity(activity, identifier.hashCode(), intent,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        Notification notification = new Notification.Builder(activity, CHANNEL)
                .setSmallIcon(R.drawable.ic_notification)
                .setContentTitle(title)
                .setContentText(body)
                .setStyle(new Notification.BigTextStyle().bigText(body))
                .setContentIntent(open)
                .setAutoCancel(true)
                .build();
        manager.notify(identifier, 0, notification);
    }

    // ── other applications ──────────────────────────────────────────────

    private static final String TERMUX = "com.termux";
    private static final String TERMUX_PERMISSION = "com.termux.permission.RUN_COMMAND";
    private static final int REQUEST_TERMUX = 3;

    public static boolean packageInstalled(String name) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return false;
        }
        try {
            activity.getPackageManager().getPackageInfo(name, 0);
            return true;
        } catch (PackageManager.NameNotFoundException e) {
            return false;
        }
    }

    /** viewPath, always with the system's list of applications. */
    public static String choosePath(String path) {
        return view(path, true, "", 0);
    }

    /** viewPath in one application: `component` is "package/class". */
    public static String viewPathWith(String path, String component) {
        return view(path, false, component, 0);
    }

    /** viewPathWith, at a line (1-based) when the application can jump to one. */
    public static String viewPathAt(String path, String component, int line) {
        return view(path, false, component, line);
    }

    /**
     * What the list of editors is asked with: a plain text file and the
     * source files that the editors of one language register for only
     * (Pydroid, Cxxdroid and Jvdroid match their own types and extensions).
     */
    private static final String[][] EDITOR_PROBES = {
        {"a.txt", "text/plain"}, {"a.md", "text/markdown"}, {"a.py", "text/x-python"},
        {"a.c", "text/x-csrc"}, {"a.cpp", "text/x-c++src"}, {"a.h", "text/x-chdr"},
        {"a.java", "text/x-java"}, {"a.js", "application/javascript"},
        {"a.json", "application/json"}, {"a.sh", "application/x-sh"},
    };

    /**
     * The applications that edit or open a text or source file, one
     * "label\tpackage/class" per line, sorted by label: the editors
     * Options › Integrations offers. Termux is left out: it copies the file
     * it is given into its own storage and edits the copy.
     */
    public static String viewApps() {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return "";
        }
        PackageManager manager = activity.getPackageManager();
        java.util.TreeMap<String, String> apps = new java.util.TreeMap<>();
        java.util.HashSet<String> seen = new java.util.HashSet<>();
        for (String[] probe : EDITOR_PROBES) {
            Uri uri = DocumentsContract.buildDocumentUri(
                    activity.getPackageName() + ".documents", "repositories/" + probe[0]);
            for (String action : new String[] {Intent.ACTION_EDIT, Intent.ACTION_VIEW}) {
                Intent intent = new Intent(action).setDataAndType(uri, probe[1]);
                for (android.content.pm.ResolveInfo info : manager.queryIntentActivities(
                        intent, PackageManager.MATCH_DEFAULT_ONLY)) {
                    if (info.activityInfo == null
                            || activity.getPackageName().equals(info.activityInfo.packageName)
                            || TERMUX.equals(info.activityInfo.packageName)) {
                        continue;
                    }
                    String component =
                            info.activityInfo.packageName + "/" + info.activityInfo.name;
                    if (!seen.add(component)) {
                        continue;
                    }
                    String label = String.valueOf(info.loadLabel(manager)).replace('\t', ' ')
                            .replace('\n', ' ');
                    // two activities with one label: tell them apart by package
                    apps.put(apps.containsKey(label)
                            ? label + " (" + info.activityInfo.packageName + ")" : label,
                            component);
                }
            }
        }
        StringBuilder lines = new StringBuilder();
        for (java.util.Map.Entry<String, String> app : apps.entrySet()) {
            lines.append(app.getKey()).append('\t').append(app.getValue()).append('\n');
        }
        return lines.toString();
    }

    /**
     * The launcher icon of an application ("package") or of one activity
     * ("package/class") as a PNG file in the cache, for the menus that list
     * applications. Returns its path, empty when there is no such
     * application.
     */
    public static String appIcon(String key) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return "";
        }
        PackageManager manager = activity.getPackageManager();
        try {
            android.graphics.drawable.Drawable icon = key.indexOf('/') < 0
                    ? manager.getApplicationIcon(key)
                    : manager.getActivityIcon(
                            android.content.ComponentName.unflattenFromString(key));
            // 16 px in the menu at up to four device pixels each
            final int size = 64;
            android.graphics.Bitmap bitmap = android.graphics.Bitmap.createBitmap(
                    size, size, android.graphics.Bitmap.Config.ARGB_8888);
            icon.setBounds(0, 0, size, size);
            icon.draw(new android.graphics.Canvas(bitmap));
            File directory = new File(activity.getCacheDir(), "app-icons");
            directory.mkdirs();
            File file = new File(directory, key.replace('/', '_') + ".png");
            try (java.io.FileOutputStream out = new java.io.FileOutputStream(file)) {
                bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, out);
            }
            bitmap.recycle();
            return file.getPath();
        } catch (PackageManager.NameNotFoundException | java.io.IOException
                | RuntimeException e) {
            return "";
        }
    }

    /**
     * Opens a file in the application the user picks for it and a folder in
     * the file manager. Other applications cannot read Corvene's storage, so
     * the intent carries a document of CorveneDocumentsProvider with a grant
     * to read and write it. Returns an error message, empty when it worked.
     */
    public static String viewPath(String path) {
        return view(path, false, "", 0);
    }

    /** Markor's `Document.EXTRA_FILE_LINE_NUMBER`: the line to show, from 0. */
    private static final String EXTRA_LINE = "EXTRA_FILE_LINE_NUMBER";

    private static String view(String path, boolean chooser, String component, int line) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return "Corvene is not open.";
        }
        File file = new File(path);
        String id = CorveneDocumentsProvider.idOf(activity, file);
        if (id == null) {
            return "Other applications cannot open " + path + ".";
        }
        Intent intent = new Intent(Intent.ACTION_VIEW);
        if (file.isDirectory()) {
            // the file manager shows shared storage under its own provider
            Uri uri = id.equals("shared") || id.startsWith("shared/")
                    ? DocumentsContract.buildDocumentUri(EXTERNAL_STORAGE,
                            "primary:" + (id.length() > 7 ? id.substring(7) : ""))
                    : DocumentsContract.buildDocumentUri(
                            activity.getPackageName() + ".documents", id);
            intent.setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR);
        } else {
            Uri uri = DocumentsContract.buildDocumentUri(
                    activity.getPackageName() + ".documents", id);
            intent.setDataAndType(uri, mimeType(file));
            android.content.ComponentName name = component.isEmpty()
                    ? null : android.content.ComponentName.unflattenFromString(component);
            if (name != null) {
                // The application's own filters decide: it edits the file
                // when it says it can, else views it. One that takes neither
                // for this type of file (an editor of one language given
                // another's) would be refused the intent, so the system's
                // list of applications is shown instead.
                PackageManager manager = activity.getPackageManager();
                String action = null;
                for (String candidate : new String[] {Intent.ACTION_EDIT, Intent.ACTION_VIEW}) {
                    Intent probe = new Intent(candidate)
                            .setDataAndType(uri, intent.getType()).setPackage(name.getPackageName());
                    if (!manager.queryIntentActivities(probe, 0).isEmpty()) {
                        action = candidate;
                        break;
                    }
                }
                if (action == null) {
                    chooser = true;
                } else {
                    intent.setAction(action).setComponent(name);
                    if (line > 0) {
                        intent.putExtra(EXTRA_LINE, line - 1);
                    }
                }
            }
        }
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION
                | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        try {
            activity.startActivity(chooser ? Intent.createChooser(intent, null) : intent);
            return "";
        } catch (ActivityNotFoundException e) {
            return "No application on this device can open " + file.getName() + ".";
        } catch (RuntimeException e) {
            return String.valueOf(e.getMessage());
        }
    }

    /**
     * Sends a file to another application (ACTION_SEND through the system's
     * share sheet), as a document of CorveneDocumentsProvider it may read.
     * Returns an error message, empty when the sheet opened.
     */
    public static String sharePath(String path) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return "Corvene is not open.";
        }
        File file = new File(path);
        String id = CorveneDocumentsProvider.idOf(activity, file);
        if (id == null || !file.isFile()) {
            return "Other applications cannot open " + path + ".";
        }
        Uri uri = DocumentsContract.buildDocumentUri(
                activity.getPackageName() + ".documents", id);
        Intent intent = new Intent(Intent.ACTION_SEND)
                .setType(mimeType(file))
                .putExtra(Intent.EXTRA_STREAM, uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        // the grant follows the clip data, not the extra
        intent.setClipData(android.content.ClipData.newRawUri(file.getName(), uri));
        try {
            activity.startActivity(Intent.createChooser(intent, null));
            return "";
        } catch (RuntimeException e) {
            return String.valueOf(e.getMessage());
        }
    }

    /** By extension; a file without a known one is text unless it has a NUL. */
    private static String mimeType(File file) {
        String name = file.getName();
        int dot = name.lastIndexOf('.');
        if (dot >= 0) {
            String type = MimeTypeMap.getSingleton()
                    .getMimeTypeFromExtension(name.substring(dot + 1).toLowerCase());
            if (type != null) {
                return type;
            }
        }
        byte[] head = new byte[8000];
        try (InputStream in = new FileInputStream(file)) {
            int read = in.read(head);
            for (int i = 0; i < read; i++) {
                if (head[i] == 0) {
                    return "application/octet-stream";
                }
            }
        } catch (IOException e) {
            return "application/octet-stream";
        }
        return "text/plain";
    }

    /**
     * A Termux session in `directory`, through Termux's RUN_COMMAND intent
     * (https://github.com/termux/termux-app/wiki/RUN_COMMAND-Intent): needs
     * its permission, which Android asks for here, and
     * `allow-external-apps = true` in ~/.termux/termux.properties. Returns an
     * error message, empty when the intent was sent.
     */
    public static String openTermux(String directory) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return "Corvene is not open.";
        }
        if (activity.checkSelfPermission(TERMUX_PERMISSION)
                != PackageManager.PERMISSION_GRANTED) {
            activity.runOnUiThread(() -> activity.requestPermissions(
                    new String[] {TERMUX_PERMISSION}, REQUEST_TERMUX));
            return "Allow Corvene to run commands in Termux, then try again.";
        }
        Intent intent = new Intent("com.termux.RUN_COMMAND")
                .setClassName(TERMUX, "com.termux.app.RunCommandService")
                .putExtra("com.termux.RUN_COMMAND_PATH",
                        "/data/data/com.termux/files/usr/bin/login")
                .putExtra("com.termux.RUN_COMMAND_WORKDIR", directory)
                .putExtra("com.termux.RUN_COMMAND_BACKGROUND", false)
                // switch to the new session and open Termux
                .putExtra("com.termux.RUN_COMMAND_SESSION_ACTION", "0");
        try {
            activity.startService(intent);
            return "";
        } catch (RuntimeException e) {
            return "Could not reach Termux: " + e.getMessage();
        }
    }

    /**
     * Runs `program` (a name in Termux's bin directory) with `arguments`
     * (one per line) in a new Termux session in `directory`, like openTermux.
     */
    public static String runTermux(String program, String arguments, String directory) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return "Corvene is not open.";
        }
        if (activity.checkSelfPermission(TERMUX_PERMISSION)
                != PackageManager.PERMISSION_GRANTED) {
            activity.runOnUiThread(() -> activity.requestPermissions(
                    new String[] {TERMUX_PERMISSION}, REQUEST_TERMUX));
            return "Allow Corvene to run commands in Termux, then try again.";
        }
        Intent intent = new Intent("com.termux.RUN_COMMAND")
                .setClassName(TERMUX, "com.termux.app.RunCommandService")
                .putExtra("com.termux.RUN_COMMAND_PATH", TERMUX_BIN + program)
                .putExtra("com.termux.RUN_COMMAND_ARGUMENTS",
                        arguments.isEmpty() ? new String[0] : arguments.split("\n"))
                .putExtra("com.termux.RUN_COMMAND_WORKDIR", directory)
                .putExtra("com.termux.RUN_COMMAND_BACKGROUND", false)
                .putExtra("com.termux.RUN_COMMAND_SESSION_ACTION", "0");
        try {
            activity.startService(intent);
            return "";
        } catch (RuntimeException e) {
            return "Could not reach Termux: " + e.getMessage();
        }
    }

    private static final String TERMUX_BIN = "/data/data/com.termux/files/usr/bin/";
    private static final String TERMUX_EDITORS = "termux_editors";
    private static final String TERMUX_EDITORS_RESULT =
            "com.wasimaster.corvene.TERMUX_EDITORS_RESULT";

    /**
     * Which of `candidates` (program names, separated by spaces) Termux has
     * installed, one per line. Termux answers a background command through a
     * PendingIntent, so this waits (never call it on the main thread) and
     * remembers the answer for the times Termux does not give one: "?" when
     * it never has (the permission or `allow-external-apps` is missing).
     */
    public static String termuxEditors(String candidates) {
        final CorveneActivity activity = instance;
        if (activity == null) {
            return "?";
        }
        android.content.SharedPreferences preferences =
                activity.getSharedPreferences(PREFERENCES, MODE_PRIVATE);
        String known = preferences.getString(TERMUX_EDITORS, "?");
        if (!candidates.matches("[a-z ]+") || Looper.myLooper() == Looper.getMainLooper()
                || activity.checkSelfPermission(TERMUX_PERMISSION)
                        != PackageManager.PERMISSION_GRANTED) {
            return known;
        }
        final java.util.concurrent.CountDownLatch done = new java.util.concurrent.CountDownLatch(1);
        final String[] found = {null};
        android.content.BroadcastReceiver receiver = new android.content.BroadcastReceiver() {
            @Override
            public void onReceive(android.content.Context context, Intent intent) {
                android.os.Bundle result = intent.getBundleExtra("result");
                if (result != null && result.getInt("exitCode", -1) == 0) {
                    found[0] = String.valueOf(result.getString("stdout", "")).trim();
                }
                done.countDown();
            }
        };
        android.content.IntentFilter filter = new android.content.IntentFilter(TERMUX_EDITORS_RESULT);
        if (android.os.Build.VERSION.SDK_INT >= 33) {
            activity.registerReceiver(receiver, filter, android.content.Context.RECEIVER_NOT_EXPORTED);
        } else {
            activity.registerReceiver(receiver, filter);
        }
        try {
            // Termux fills the result in: the PendingIntent has to be mutable
            int flags = PendingIntent.FLAG_UPDATE_CURRENT
                    | (android.os.Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0);
            PendingIntent reply = PendingIntent.getBroadcast(activity, 0,
                    new Intent(TERMUX_EDITORS_RESULT).setPackage(activity.getPackageName()), flags);
            activity.startService(new Intent("com.termux.RUN_COMMAND")
                    .setClassName(TERMUX, "com.termux.app.RunCommandService")
                    .putExtra("com.termux.RUN_COMMAND_PATH", TERMUX_BIN + "sh")
                    .putExtra("com.termux.RUN_COMMAND_ARGUMENTS", new String[] {"-c",
                            "for e in " + candidates
                                    + "; do command -v $e >/dev/null && echo $e; done; true"})
                    .putExtra("com.termux.RUN_COMMAND_BACKGROUND", true)
                    .putExtra("com.termux.RUN_COMMAND_PENDING_INTENT", reply));
            done.await(2500, java.util.concurrent.TimeUnit.MILLISECONDS);
        } catch (RuntimeException | InterruptedException e) {
            Log.w("corvene", "termux editors: " + e);
        } finally {
            activity.unregisterReceiver(receiver);
        }
        if (found[0] == null) {
            return known;
        }
        preferences.edit().putString(TERMUX_EDITORS, found[0]).apply();
        return found[0];
    }

    // ── network operations ──────────────────────────────────────────────

    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static boolean transferServiceStarted;
    private static final Runnable START_TRANSFER_SERVICE = () -> {
        final CorveneActivity activity = instance;
        if (activity == null || transferServiceStarted) {
            return;
        }
        try {
            activity.startForegroundService(new Intent(activity, CorveneTransferService.class));
            transferServiceStarted = true;
        } catch (RuntimeException e) {
            // not allowed from the background (a fetch WorkManager started)
            Log.w("corvene", "transfer service: " + e);
        }
    };

    /**
     * A clone, fetch, pull or push runs (or the last one ended): while one
     * does, CorveneTransferService keeps the process from being stopped when
     * the user leaves the application. Short operations end before the
     * service is started.
     */
    public static void transferActive(boolean active) {
        MAIN.removeCallbacks(START_TRANSFER_SERVICE);
        if (active) {
            MAIN.postDelayed(START_TRANSFER_SERVICE, 1500);
            return;
        }
        MAIN.post(() -> {
            final CorveneActivity activity = instance;
            if (activity == null || !transferServiceStarted) {
                return;
            }
            transferServiceStarted = false;
            try {
                activity.startService(new Intent(activity, CorveneTransferService.class)
                        .setAction(CorveneTransferService.ACTION_STOP));
            } catch (RuntimeException e) {
                activity.stopService(new Intent(activity, CorveneTransferService.class));
            }
        });
    }

    /**
     * The background fetch in a process without an activity: true when a
     * fetch ran. Does nothing once an activity was created.
     */
    static native boolean nativeHeadlessFetch(Context context, String filesDir);

    /** Stops nativeHeadlessFetch and waits for it to return. */
    static native void nativeEndHeadless();

    /** WorkManager's hourly work: fetch like the background fetcher. */
    static native boolean nativeBackgroundFetch();

    /** Whether a network operation runs. */
    static native boolean nativeNetworkBusy();

    static native void nativeNotificationClicked(String identifier, String payload);

    static native void nativePathPicked(String path, String error);

    static native void nativeCommitText(String text);

    static native void nativeSetComposingText(String text);

    static native void nativeFinishComposing();

    static native void nativeDeleteSurrounding(int before, int after);

    static native void nativeKey(int keyCode, boolean down, int metaState, int unicode);

    static native void nativeOpenUrl(String url);

    static native void nativeInsets(int left, int top, int right, int bottom);

    /** The view the input method is connected to. It draws nothing. */
    private static final class InputView extends View {
        InputView(Context context) {
            super(context);
            setFocusable(true);
            setFocusableInTouchMode(true);
        }

        @Override
        public boolean onCheckIsTextEditor() {
            return true;
        }

        @Override
        public InputConnection onCreateInputConnection(EditorInfo info) {
            // Plain text without suggestions: the native text fields own the
            // text, so the keyboard has nothing to correct against.
            info.inputType = InputType.TYPE_CLASS_TEXT
                    | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                    | InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD;
            info.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN
                    | EditorInfo.IME_FLAG_NO_EXTRACT_UI
                    | EditorInfo.IME_ACTION_NONE;
            return new Connection(this);
        }
    }

    private static final class Connection extends BaseInputConnection {
        Connection(View view) {
            super(view, false);
        }

        @Override
        public boolean commitText(CharSequence text, int newCursorPosition) {
            nativeCommitText(text.toString());
            return true;
        }

        @Override
        public boolean setComposingText(CharSequence text, int newCursorPosition) {
            nativeSetComposingText(text.toString());
            return true;
        }

        @Override
        public boolean finishComposingText() {
            nativeFinishComposing();
            return true;
        }

        @Override
        public boolean deleteSurroundingText(int beforeLength, int afterLength) {
            nativeDeleteSurrounding(beforeLength, afterLength);
            return true;
        }

        @Override
        public boolean sendKeyEvent(KeyEvent event) {
            int action = event.getAction();
            if (action == KeyEvent.ACTION_DOWN || action == KeyEvent.ACTION_UP) {
                nativeKey(event.getKeyCode(), action == KeyEvent.ACTION_DOWN,
                        event.getMetaState(), event.getUnicodeChar());
            }
            return true;
        }
    }
}
