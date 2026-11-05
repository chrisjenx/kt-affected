package demo

import kotlin.reflect.full.declaredMemberFunctions
import kotlin.test.Test
import kotlin.test.assertEquals

class OtherShapeTest {
    @Test
    fun otherHasOneOperation() {
        assertEquals(1, Other::class.declaredMemberFunctions.size)
    }
}
