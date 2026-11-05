package demo

import kotlin.test.Test

class BookStoreTest {
    @Test
    fun lists() {
        BookStore(testDb().bookQueries).names()
    }
}
