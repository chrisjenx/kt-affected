package demo.guards

import java.io.File
import kotlin.test.Test

/** Reads production sources as text: the graph never reaches it. */
class CborGuardTest {
    @Test
    fun noRawCbor() {
        File("../orders/src").walk().forEach { check(!it.readText().contains("RawCbor")) }
    }
}
