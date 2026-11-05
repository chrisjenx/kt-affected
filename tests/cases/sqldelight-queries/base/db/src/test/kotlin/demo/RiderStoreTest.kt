package demo

import kotlin.test.Test

class RiderStoreTest {
    @Test
    fun finds() {
        RiderStore(testDb().riderQueries).named("Ada")
    }
}
