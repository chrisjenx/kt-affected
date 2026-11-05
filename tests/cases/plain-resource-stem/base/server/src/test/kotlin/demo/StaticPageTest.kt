package demo

import kotlin.test.Test
import kotlin.test.assertNotNull

class StaticPageTest {
    @Test
    fun servesTheIndexPage() {
        assertNotNull(javaClass.getResource("/index.html"))
    }
}
