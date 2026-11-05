package demo

import org.koin.core.context.startKoin
import org.koin.test.KoinTest
import org.koin.test.inject
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals

class HorseTest : KoinTest {
    private val horses: HorseRepo by inject()

    @BeforeTest
    fun start() {
        startKoin { modules(appModule) }
    }

    @Test
    fun loadsAHorse() {
        assertEquals("row2", horses.horse(2))
    }
}
