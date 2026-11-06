package demo.konsist

import kotlin.test.Test

class ArchitectureTest : KonsistRules() {
    @Test
    fun repositoriesAreInterfaces() = repositoriesAreInterfacesRule()
}
