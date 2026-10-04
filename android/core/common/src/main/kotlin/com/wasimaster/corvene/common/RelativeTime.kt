package com.wasimaster.corvene.common

import android.text.format.DateUtils

/**
 * "5 min. ago", "Yesterday", "Mar 3": [seconds] (Unix time, as the engine
 * sends dates) relative to [nowMillis]. Screens take `now` as a parameter so
 * previews and screenshot tests stay fixed.
 */
fun relativeTime(seconds: Long, nowMillis: Long): String =
    DateUtils.getRelativeTimeSpanString(
        seconds * MILLIS_PER_SECOND,
        nowMillis,
        DateUtils.MINUTE_IN_MILLIS,
        DateUtils.FORMAT_ABBREV_RELATIVE,
    ).toString()

private const val MILLIS_PER_SECOND = 1000L
