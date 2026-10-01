package com.wasimaster.corvane;

import android.app.NativeActivity;
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
    protected void onCreate(Bundle savedInstanceState) {
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
        String url = intent.getDataString();
        if (url != null) {
            nativeOpenUrl(url);
        }
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
