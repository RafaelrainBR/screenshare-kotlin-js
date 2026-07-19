package screenshare.clientkmp.services

import io.ktor.client.HttpClient

expect fun createPlatformHttpClient(): HttpClient
