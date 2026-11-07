package demo.books

import kotlin.test.Test
import kotlin.test.assertEquals

class BookShelfTest {
    @Test
    fun countsEveryAdd() {
        val shelf = BookShelf()
        shelf.add("Ada")
        shelf.add("Grace")
        assertEquals(2, shelf.count())
    }
}
