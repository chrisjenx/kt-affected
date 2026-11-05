package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class InvoiceTest {
    private val ext = ServicesExtension()

    @Test
    fun issues() {
        assertEquals("invoice:5", ext.services.invoices.issue(5))
    }
}
