package demo

import kotlin.reflect.full.declaredMemberFunctions
import kotlin.test.Test
import kotlin.test.assertEquals

class RepoShapeTest {
    @Test
    fun repoHasOneOperation() {
        assertEquals(1, Repo::class.declaredMemberFunctions.size)
    }
}
