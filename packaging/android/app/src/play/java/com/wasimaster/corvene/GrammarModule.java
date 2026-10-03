package com.wasimaster.corvene;

import android.app.Activity;
import android.content.Context;
import android.content.IntentSender;
import android.content.pm.PackageManager;

import com.google.android.play.core.splitcompat.SplitCompat;
import com.google.android.play.core.splitinstall.SplitInstallManager;
import com.google.android.play.core.splitinstall.SplitInstallManagerFactory;
import com.google.android.play.core.splitinstall.SplitInstallRequest;
import com.google.android.play.core.splitinstall.SplitInstallStateUpdatedListener;
import com.google.android.play.core.splitinstall.model.SplitInstallSessionStatus;

import java.io.File;
import java.util.Collections;

/**
 * The tree-sitter grammars in the play flavour: the on-demand feature module
 * "grammars" (packaging/android/grammars), which Google Play installs when
 * the user asks for it in Options › Advanced. The module is native libraries
 * only; the native side opens them by path once it knows where they are
 * (directory).
 */
final class GrammarModule {
    private static final String NAME = "grammars";
    /** The module's index, see grammars/build.gradle.kts. */
    private static final String INDEX = "libcorvene_ts_index.so";
    private static final int REQUEST_CONFIRMATION = 4;

    private static SplitInstallManager manager;
    private static Activity current;

    private GrammarModule() {}

    /** From attachBaseContext: lets a freshly installed module be used at once. */
    static void attach(Context context) {
        SplitCompat.installActivity(context);
    }

    private static SplitInstallManager manager(Context context) {
        if (manager == null) {
            manager = SplitInstallManagerFactory.create(context.getApplicationContext());
            SplitInstallStateUpdatedListener listener = state -> {
                if (!state.moduleNames().contains(NAME)) {
                    return;
                }
                switch (state.status()) {
                    case SplitInstallSessionStatus.DOWNLOADING:
                        CorveneActivity.nativeGrammarModule(0, state.bytesDownloaded(),
                                state.totalBytesToDownload(), "");
                        break;
                    case SplitInstallSessionStatus.REQUIRES_USER_CONFIRMATION:
                        // a large download: Play asks the user first
                        try {
                            if (current != null) {
                                manager.startConfirmationDialogForResult(state, current,
                                        REQUEST_CONFIRMATION);
                            }
                        } catch (IntentSender.SendIntentException e) {
                            CorveneActivity.nativeGrammarModule(2, 0, 0,
                                    String.valueOf(e.getMessage()));
                        }
                        break;
                    case SplitInstallSessionStatus.INSTALLED:
                        if (current != null) {
                            SplitCompat.installActivity(current);
                        }
                        CorveneActivity.nativeGrammarModule(1, 0, 0, "");
                        break;
                    case SplitInstallSessionStatus.FAILED:
                        CorveneActivity.nativeGrammarModule(2, 0, 0,
                                "Google Play could not install the grammars (error "
                                        + state.errorCode() + ").");
                        break;
                    case SplitInstallSessionStatus.CANCELED:
                        CorveneActivity.nativeGrammarModule(2, 0, 0,
                                "The installation was cancelled.");
                        break;
                    default:
                        break;
                }
            };
            manager.registerListener(listener);
        }
        return manager;
    }

    /**
     * The folder with the module's libraries, empty when it is not
     * installed. Play installs a split's libraries into the application's
     * library folder; until the next start they can also sit in SplitCompat's
     * own folder under files/.
     */
    static String directory(Context context) {
        try {
            String installed = context.getPackageManager()
                    .getApplicationInfo(context.getPackageName(), 0).nativeLibraryDir;
            if (installed != null && new File(installed, INDEX).isFile()) {
                return installed;
            }
        } catch (PackageManager.NameNotFoundException e) {
            // our own package
        }
        File found = find(new File(context.getFilesDir(), "splitcompat"), 0);
        return found != null ? found.getAbsolutePath() : "";
    }

    private static File find(File directory, int depth) {
        File[] children = depth > 6 ? null : directory.listFiles();
        if (children == null) {
            return null;
        }
        for (File child : children) {
            if (child.isFile() && child.getName().equals(INDEX)) {
                return directory;
            }
        }
        for (File child : children) {
            if (child.isDirectory()) {
                File found = find(child, depth + 1);
                if (found != null) {
                    return found;
                }
            }
        }
        return null;
    }

    static void install(Activity activity) {
        current = activity;
        SplitInstallManager manager = manager(activity);
        if (manager.getInstalledModules().contains(NAME)) {
            CorveneActivity.nativeGrammarModule(1, 0, 0, "");
            return;
        }
        manager.startInstall(SplitInstallRequest.newBuilder().addModule(NAME).build())
                .addOnFailureListener(e -> CorveneActivity.nativeGrammarModule(2, 0, 0,
                        "Google Play could not install the grammars: " + e.getMessage()));
    }

    /** Play removes the module some time later, in the background. */
    static void uninstall(Context context) {
        manager(context).deferredUninstall(Collections.singletonList(NAME));
    }
}
