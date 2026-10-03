package com.wasimaster.corvene.ffi

import org.junit.Assert.assertEquals
import org.junit.Test

/** The callbacks become versions and queued requests, in order, and nothing else. */
class HostEventsBridgeTest {

    @Test
    fun `callbacks map to versions and requests`() {
        val versions = mutableListOf<Long>()
        val requests = mutableListOf<HostRequest>()
        val bridge = HostEventsBridge(onStateChanged = { versions += it }, send = { requests += it })
        bridge.stateChanged(7u)
        bridge.pickPaths(3u, directories = true, multiple = false, prompt = "Add")
        bridge.toast("hi")
        bridge.quit()
        assertEquals(listOf(7L), versions)
        assertEquals(
            listOf(HostRequest.PickPaths(3u, true, false, "Add"), HostRequest.Toast("hi"), HostRequest.Quit),
            requests,
        )
    }
}
