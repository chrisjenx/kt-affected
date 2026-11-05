package demo

import kotlin.test.Test

class HorseDaoTest {
    @Test
    fun runs() {
        HorseDao().count()
    }
}
