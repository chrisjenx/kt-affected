package demo.library

interface LoanRepo {
    fun open(member: String): Int
}

class MemoryLoanRepo : LoanRepo {
    override fun open(member: String): Int = member.length
}

class Loans(private val repo: LoanRepo) {
    fun count(member: String): Int = repo.open(member)

    fun overdueFine(days: Int): Int = days * 10
}

fun loans(): Loans = Loans(MemoryLoanRepo())

object Reports {
    fun yearly(): String = "none"
}
