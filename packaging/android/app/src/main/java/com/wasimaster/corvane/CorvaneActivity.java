package com.wasimaster.corvane;

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
 * Corvane's one activity. The application is native code: NativeActivity
 * loads libcorvane.so and hands it the window, the input queue and the
 * lifecycle. What a NativeActivity does not get is text: the input method
 * only talks to a view with an InputConnection, so an invisible focused view
 * provides one and passes what the keyboard does to the native side
 * (crates/corvane/src/android.rs).
 */
public class CorvaneActivity extends NativeActivity {
    static {
        // NativeActivity opens the library with dlopen, which does not make
        // its JNI functions known to this class loader.
        System.loadLibrary("corvane");
    }

    private static CorvaneActivity instance;
    private InputView inputView;

    @Override
    protected void attachBaseContext(Context base) {
        super.attachBaseContext(base);
        GrammarModule.attach(this);
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // a background fetch WorkManager started in this process without an
        // activity (CorvaneFetchWorker) has to let go before the native
        // side starts
        nativeEndHeadless();
        super.onCreate(savedInstanceState);
        instance = this;
        // NativeActivity leaves the keyboard's initial state to the system,
        // which opens it for a focused editor on some devices; it should
        // only come up when a text field is focused (showKeyboard).
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_HIDDEN
                | WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        inputView = new InputView(this);
        addContentView(inputView, new ViewGroup.LayoutParams(1, 1));
        inputView.requestFocus();
        // a link is read by the platform (`getIntent().getDataString()`)
        if (ACTION_NOTIFICATION.equals(getIntent().getAction())) {
            handleIntent(getIntent());
        }
        CorvaneFetchWorker.schedule(this);
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
        final CorvaneActivity activity = instance;
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
     * comes through nativePathPicked: a path Corvane can use directly, or an
     * error for the user.
     */
    public static void pickFolder() {
        final CorvaneActivity activity = instance;
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

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
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
     * A picked folder as a path: one of Corvane's own (through its documents
     * provider), one on shared storage when "All files access" is granted,
     * else, for a Git repository, a copy imported into Corvane's storage.
     */
    private void resolvePickedFolder(Uri tree) {
        String documentId = DocumentsContract.getTreeDocumentId(tree);
        String authority = tree.getAuthority();
        if ((getPackageName() + ".documents").equals(authority)) {
            File folder = CorvaneDocumentsProvider.fileOf(
                    CorvaneDocumentsProvider.baseDirectory(this), documentId);
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
                        ? "Corvane can only use folders in its own storage. To use a folder "
                                + "on shared storage in place, allow \"All files access\" first."
                        : "Corvane can only use folders in its own storage. A folder that "
                                + "is a Git repository is imported (copied) when picked.");
                return;
            }
            File base = CorvaneDocumentsProvider.baseDirectory(this);
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
        final CorvaneActivity activity = instance;
        return activity == null ? "" : GrammarModule.directory(activity);
    }

    public static void installGrammarModule() {
        final CorvaneActivity activity = instance;
        if (activity != null) {
            activity.runOnUiThread(() -> GrammarModule.install(activity));
        }
    }

    public static void uninstallGrammarModule() {
        final CorvaneActivity activity = instance;
        if (activity != null) {
            GrammarModule.uninstall(activity);
        }
    }

    /** 0: progress, 1: installed, 2: failed (with a message). */
    static native void nativeGrammarModule(int status, long received, long total, String error);

    /** This build declares MANAGE_EXTERNAL_STORAGE (the foss flavour). */
    public static boolean canRequestAllFilesAccess() {
        final CorvaneActivity activity = instance;
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

    /** The system's "All files access" page for Corvane. */
    public static void requestAllFilesAccess() {
        final CorvaneActivity activity = instance;
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

    private static final String ACTION_NOTIFICATION = "com.wasimaster.corvane.NOTIFICATION";
    private static final String EXTRA_IDENTIFIER = "identifier";
    private static final String EXTRA_PAYLOAD = "payload";
    private static final String CHANNEL = "pull-requests";
    private static final String PREFERENCES = "corvane";
    private static final String ASKED_NOTIFICATIONS = "asked-notifications";
    private static final int REQUEST_NOTIFICATIONS = 2;

    /**
     * A page in a Custom Tab: the browser's tab drawn over Corvane, so
     * signing in does not leave the application. Custom Tabs are an intent
     * convention; a browser without them opens the page normally.
     */
    public static void openCustomTab(final String url) {
        final CorvaneActivity activity = instance;
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
        String url = intent.getDataString();
        if (url != null) {
            nativeOpenUrl(url);
        }
    }

    /** 0: not asked yet, 1: allowed, 2: denied. */
    public static int notificationPermission() {
        final CorvaneActivity activity = instance;
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
        final CorvaneActivity activity = instance;
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
        final CorvaneActivity activity = instance;
        if (activity == null) {
            return;
        }
        activity.runOnUiThread(() -> activity.startActivity(
                new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                        .putExtra(Settings.EXTRA_APP_PACKAGE, activity.getPackageName())));
    }

    /** Posts a notification; tapping it opens Corvane with its payload. */
    public static void showNotification(String identifier, String title, String body,
            String payload) {
        final CorvaneActivity activity = instance;
        if (activity == null) {
            return;
        }
        NotificationManager manager = activity.getSystemService(NotificationManager.class);
        manager.createNotificationChannel(new NotificationChannel(CHANNEL,
                activity.getString(R.string.channel_pull_requests),
                NotificationManager.IMPORTANCE_DEFAULT));
        Intent intent = new Intent(activity, CorvaneActivity.class)
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
        final CorvaneActivity activity = instance;
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
        return view(path, true, "");
    }

    /** viewPath in one application: `component` is "package/class". */
    public static String viewPathWith(String path, String component) {
        return view(path, false, component);
    }

    /**
     * The applications that open a text file, one "label\tpackage/class" per
     * line, sorted by label: the editors Options › Integrations offers.
     */
    public static String viewApps() {
        final CorvaneActivity activity = instance;
        if (activity == null) {
            return "";
        }
        PackageManager manager = activity.getPackageManager();
        Intent intent = new Intent(Intent.ACTION_VIEW).setDataAndType(
                DocumentsContract.buildDocumentUri(
                        activity.getPackageName() + ".documents", "repositories/a.txt"),
                "text/plain");
        java.util.TreeMap<String, String> apps = new java.util.TreeMap<>();
        for (android.content.pm.ResolveInfo info
                : manager.queryIntentActivities(intent, PackageManager.MATCH_DEFAULT_ONLY)) {
            if (info.activityInfo == null
                    || activity.getPackageName().equals(info.activityInfo.packageName)) {
                continue;
            }
            String label = String.valueOf(info.loadLabel(manager)).replace('\t', ' ')
                    .replace('\n', ' ');
            String component = info.activityInfo.packageName + "/" + info.activityInfo.name;
            // two activities with one label: tell them apart by package
            apps.put(apps.containsKey(label)
                    ? label + " (" + info.activityInfo.packageName + ")" : label, component);
        }
        StringBuilder lines = new StringBuilder();
        for (java.util.Map.Entry<String, String> app : apps.entrySet()) {
            lines.append(app.getKey()).append('\t').append(app.getValue()).append('\n');
        }
        return lines.toString();
    }

    /**
     * Opens a file in the application the user picks for it and a folder in
     * the file manager. Other applications cannot read Corvane's storage, so
     * the intent carries a document of CorvaneDocumentsProvider with a grant
     * to read and write it. Returns an error message, empty when it worked.
     */
    public static String viewPath(String path) {
        return view(path, false, "");
    }

    private static String view(String path, boolean chooser, String component) {
        final CorvaneActivity activity = instance;
        if (activity == null) {
            return "Corvane is not open.";
        }
        File file = new File(path);
        String id = CorvaneDocumentsProvider.idOf(activity, file);
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
            if (!component.isEmpty()) {
                intent.setComponent(android.content.ComponentName.unflattenFromString(component));
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
     * share sheet), as a document of CorvaneDocumentsProvider it may read.
     * Returns an error message, empty when the sheet opened.
     */
    public static String sharePath(String path) {
        final CorvaneActivity activity = instance;
        if (activity == null) {
            return "Corvane is not open.";
        }
        File file = new File(path);
        String id = CorvaneDocumentsProvider.idOf(activity, file);
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
        final CorvaneActivity activity = instance;
        if (activity == null) {
            return "Corvane is not open.";
        }
        if (activity.checkSelfPermission(TERMUX_PERMISSION)
                != PackageManager.PERMISSION_GRANTED) {
            activity.runOnUiThread(() -> activity.requestPermissions(
                    new String[] {TERMUX_PERMISSION}, REQUEST_TERMUX));
            return "Allow Corvane to run commands in Termux, then try again.";
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

    // ── network operations ──────────────────────────────────────────────

    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static boolean transferServiceStarted;
    private static final Runnable START_TRANSFER_SERVICE = () -> {
        final CorvaneActivity activity = instance;
        if (activity == null || transferServiceStarted) {
            return;
        }
        try {
            activity.startForegroundService(new Intent(activity, CorvaneTransferService.class));
            transferServiceStarted = true;
        } catch (RuntimeException e) {
            // not allowed from the background (a fetch WorkManager started)
            Log.w("corvane", "transfer service: " + e);
        }
    };

    /**
     * A clone, fetch, pull or push runs (or the last one ended): while one
     * does, CorvaneTransferService keeps the process from being stopped when
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
            final CorvaneActivity activity = instance;
            if (activity == null || !transferServiceStarted) {
                return;
            }
            transferServiceStarted = false;
            try {
                activity.startService(new Intent(activity, CorvaneTransferService.class)
                        .setAction(CorvaneTransferService.ACTION_STOP));
            } catch (RuntimeException e) {
                activity.stopService(new Intent(activity, CorvaneTransferService.class));
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
