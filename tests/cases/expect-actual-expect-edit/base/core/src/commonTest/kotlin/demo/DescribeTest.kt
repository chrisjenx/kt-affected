package demo

import kotlin.test.Test
import kotlin.test.assertTrue

class DescribeTest {
    @Test
    fun describes() {
        assertTrue(describe().startsWith("running on"))
    }
}
