package com.wasimaster.corvene.ffi

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri

/**
 * What the engine asks about other applications, answered from the package
 * manager (the engine lists editors in Settings › Integrations from it).
 */
internal object InstalledApps {
    /** The applications that open a text file, as "label\tpackage/class". */
    fun textViewers(context: Context): List<String> {
        val probe = Intent(Intent.ACTION_VIEW).setDataAndType(Uri.parse("content://probe/file.txt"), "text/plain")
        val manager = context.packageManager
        return manager.queryIntentActivities(probe, PackageManager.MATCH_DEFAULT_ONLY)
            .map { it.activityInfo }
            .filter { it.packageName != context.packageName }
            .map { info -> "${info.loadLabel(manager)}\t${info.packageName}/${info.name}" }
            .distinct()
    }

    fun installed(context: Context, packageName: String): Boolean = try {
        context.packageManager.getPackageInfo(packageName, 0)
        true
    } catch (_: PackageManager.NameNotFoundException) {
        false
    }
}
