package demo

import kotlin.test.Test

class HorseStoreTest {
    @Test
    fun lists() {
        HorseStore(testDb().horseQueries).names()
    }
}
