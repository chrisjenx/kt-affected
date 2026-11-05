package p

interface RepoApi {
    fun foo(): Int
    fun bar(): Int
}

class RepoImpl : RepoApi {
    override fun foo(): Int = 41
    override fun bar(): Int = 2
}
