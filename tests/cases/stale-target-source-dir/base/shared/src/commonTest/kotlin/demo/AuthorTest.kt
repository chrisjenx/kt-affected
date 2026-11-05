package demo

import kotlin.test.Test
import kotlin.test.assertEquals

class AuthorTest {
    @Test
    fun takesTheFirstLetter() {
        assertEquals("A", Author("Ann").initials())
    }
}
