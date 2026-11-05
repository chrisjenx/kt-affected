package demo

import org.koin.core.context.startKoin
import org.koin.test.KoinTest
import org.koin.test.get
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals

class UserCountTest : KoinTest {
    @BeforeTest
    fun start() {
        startKoin { modules(appModule) }
    }

    @Test
    fun counts() {
        assertEquals(1, get<UserRepo>().userCount())
    }
}
