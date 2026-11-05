package demo

import kotlin.test.Test

class SuppressedTest {
    @Test
    fun runs() {
        Suppressed().magic()
    }
}
