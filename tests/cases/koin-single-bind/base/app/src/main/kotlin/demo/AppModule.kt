package demo

import org.koin.core.module.dsl.bind
import org.koin.core.module.dsl.singleOf
import org.koin.dsl.bind
import org.koin.dsl.module

val appModule = module {
    single { Db() }
    single { UserRepoImpl(get()) } bind UserRepo::class
    singleOf(::HorseRepoImpl) { bind<HorseRepo>() }
    factory<Clock> { FixedClock() }
}
