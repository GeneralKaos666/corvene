package com.wasimaster.corvane;

import android.app.NativeActivity;
import android.content.Context;
import android.content.Intent;
import android.os.Bundle;
import android.text.InputType;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;

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
