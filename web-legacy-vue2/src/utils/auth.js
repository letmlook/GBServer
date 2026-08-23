import Cookies from 'js-cookie'

const TokenKey = 'gbserver_token'
const NameKey = 'gbserver_username'
const serverIdKey = 'gbserver_server_id'
const expires = 30

export function getToken() {
  console.log('Getting token...')
  return Cookies.get(TokenKey)
}

export function setToken(token) {
  return Cookies.set(TokenKey, token, {expires: expires})
}

export function removeToken() {
  return Cookies.remove(TokenKey)
}

export function getName() {
  return Cookies.get(NameKey)
}

export function setName(name) {
  return Cookies.set(NameKey, name, {expires: expires})
}

export function removeName() {
  return Cookies.remove(NameKey)
}

export function getServerId() {
  return Cookies.get(serverIdKey)
}

export function setServerId(serverId) {
  return Cookies.set(serverIdKey, serverId, {expires: expires})
}

export function removeServerId() {
  return Cookies.remove(serverIdKey)
}
