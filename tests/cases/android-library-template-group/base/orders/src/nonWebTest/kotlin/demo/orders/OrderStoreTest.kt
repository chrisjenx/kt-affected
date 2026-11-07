package demo.orders

import kotlin.test.Test
import kotlin.test.assertEquals

class OrderStoreTest {
    @Test
    fun totalSumsEveryRow() {
        val store = OrderStore()
        store.put(Order("1", 100))
        store.put(Order("2", 250))
        assertEquals(350, store.total())
    }
}
