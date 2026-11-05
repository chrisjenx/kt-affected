package demo

import kotlin.test.Test
import kotlin.test.assertNotNull

class AppIconTest {
    @Test
    fun hasAnIcon() {
        assertNotNull(appIcon())
    }
}
