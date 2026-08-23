import Cookies from 'js-cookie'

const TokenKey = 'gbserver_token'
const NameKey = 'gbserver_username'
const ServerIdKey = 'gbserver_server_id'
const expires = 30

export function getToken(): string | undefined {
  return Cookies.get(TokenKey)
}

export function setToken(token: string): void {
  Cookies.set(TokenKey, token, { expires })
}

export function removeToken(): void {
  Cookies.remove(TokenKey)
}

export function getName(): string | undefined {
  return Cookies.get(NameKey)
}

export function setName(name: string): void {
  Cookies.set(NameKey, name, { expires })
}

export function removeName(): void {
  Cookies.remove(NameKey)
}

export function getServerId(): string | undefined {
  return Cookies.get(ServerIdKey)
}

export function setServerId(serverId: string): void {
  Cookies.set(ServerIdKey, serverId, { expires })
}

export function removeServerId(): void {
  Cookies.remove(ServerIdKey)
}
