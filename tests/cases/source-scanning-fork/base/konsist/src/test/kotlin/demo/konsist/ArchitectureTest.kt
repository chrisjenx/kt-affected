package demo.konsist

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.verify.assertTrue
import kotlin.test.Test

class ArchitectureTest {
    @Test
    fun repositoriesAreInterfaces() {
        Konsist.scopeFromProject()
            .classes()
            .filter { it.name.endsWith("Repository") }
            .assertTrue { it.hasInterfaceModifier }
    }
}
