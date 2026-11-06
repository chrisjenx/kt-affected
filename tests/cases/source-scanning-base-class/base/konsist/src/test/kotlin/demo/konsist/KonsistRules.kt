package demo.konsist

import com.lemonappdev.konsist.api.Konsist
import com.lemonappdev.konsist.api.verify.assertTrue

abstract class KonsistRules {
    fun repositoriesAreInterfacesRule() {
        Konsist.scopeFromProject()
            .classes()
            .filter { it.name.endsWith("Repository") }
            .assertTrue { it.hasInterfaceModifier }
    }
}
