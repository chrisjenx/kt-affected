package demo.history

import kotlin.test.Test
import kotlin.test.assertEquals

class HistoryTest {
    @Test
    fun empty() {
        assertEquals(0, History().count())
    }
}
