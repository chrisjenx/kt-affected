package demo.invoices

import kotlin.test.Test
import kotlin.test.assertEquals

class InvoiceStoreTest {
    @Test
    fun totalSumsEveryRow() {
        val store = InvoiceStore()
        store.put(Invoice("1", 100))
        store.put(Invoice("2", 250))
        assertEquals(350, store.total())
    }
}
