package demo.core

import kotlin.test.Test
import kotlin.test.assertEquals

class BooksTest {
    @Test
    fun empty() {
        assertEquals(0, Books().count())
    }
}
