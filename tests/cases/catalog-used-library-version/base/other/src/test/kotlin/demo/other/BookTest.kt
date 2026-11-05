package demo.other

import kotlin.test.Test
import kotlin.test.assertEquals

class BookTest {
    @Test
    fun keepsTitle() {
        assertEquals("Dune", Book("Dune").title)
    }
}
