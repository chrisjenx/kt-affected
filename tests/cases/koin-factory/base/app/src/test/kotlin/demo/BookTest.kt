package demo

import org.koin.core.context.startKoin
import org.koin.test.KoinTest
import org.koin.test.inject
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals

class BookTest : KoinTest {
    private val books: BookRepo by inject()

    @BeforeTest
    fun start() {
        startKoin { modules(appModule) }
    }

    @Test
    fun loadsABook() {
        assertEquals("row2", books.book(2))
    }
}
