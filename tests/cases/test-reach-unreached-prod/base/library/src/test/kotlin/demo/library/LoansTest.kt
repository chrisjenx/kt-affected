package demo.library

import kotlin.test.Test
import kotlin.test.assertEquals

class LoansTest {
    @Test
    fun counts() {
        assertEquals(3, loans().count("ann"))
    }
}
