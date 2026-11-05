package demo.app

import kotlin.test.Test
import kotlin.test.assertEquals

class InvoiceTest {
    @Test
    fun sums() {
        assertEquals(3, Invoice(listOf(1, 2)).total())
    }
}
