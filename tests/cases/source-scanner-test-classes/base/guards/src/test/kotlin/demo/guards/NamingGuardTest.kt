package demo.guards

import java.io.File
import kotlin.test.Test

class NamingGuardTest {
    @Test
    fun noManagerSuffix() {
        File("../backend-billing/src").walk().forEach { check(!it.name.endsWith("Manager.kt")) }
    }
}
