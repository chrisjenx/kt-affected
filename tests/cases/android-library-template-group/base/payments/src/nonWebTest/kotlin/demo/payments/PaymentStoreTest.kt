package demo.payments

import kotlin.test.Test
import kotlin.test.assertEquals

class PaymentStoreTest {
    @Test
    fun totalSumsEveryRow() {
        val store = PaymentStore()
        store.put(Payment("1", 100))
        store.put(Payment("2", 250))
        assertEquals(350, store.total())
    }
}
