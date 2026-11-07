package demo.invoices

import kotlin.test.Test
import kotlin.test.assertEquals

class InvoiceTest {
    @Test
    fun labelNamesTheId() {
        assertEquals("invoice 7", Invoice("7", 100).label())
    }
}
