package demo

import kotlin.test.Test

class BookDaoTest {
    @Test
    fun runs() {
        BookDao().count()
    }
}
