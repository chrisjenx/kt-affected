package demo.books

import kotlin.test.Test
import kotlin.test.assertEquals

class ShelfTest {
    @Test
    fun listsTitlesAndLines() {
        val books = listOf(Book("Dune"))
        assertEquals(listOf("Dune"), shelfTitles(books))
        assertEquals(listOf("- Dune"), catalogLines(books))
    }
}
