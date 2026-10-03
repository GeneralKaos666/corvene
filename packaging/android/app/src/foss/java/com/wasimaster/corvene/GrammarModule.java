package com.wasimaster.corvene;

import android.app.Activity;
import android.content.Context;

/**
 * The foss flavour has no Play feature module: it downloads the grammar
 * packs from Corvene's releases. The play flavour's class of this name talks
 * to Google Play.
 */
final class GrammarModule {
    private GrammarModule() {}

    static void attach(Context context) {}

    static String directory(Context context) {
        return "";
    }

    static void install(Activity activity) {
        CorveneActivity.nativeGrammarModule(2, 0, 0, "This build has no Google Play module.");
    }

    static void uninstall(Context context) {}
}
