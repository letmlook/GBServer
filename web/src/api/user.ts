import { request } from '@/utils/request'
import type { WvpResult } from '@/types/api'
import * as md5ns from 'js-md5'
const md5 = (md5ns as unknown as { default?: (s: string) => string; (s: string): string }).default
  ?? (md5ns as unknown as (s: string) => string)

export interface LoginPayload {
  username: string
  password: string
}

export interface LoginResult {
  id: number
  accessToken: string
  username: string
  serverId: string
}

export interface UserInfoResult {
  id: number
  username: string
  role?: { id: number; name: string; authority?: string }
  pushKey?: string
  createTime?: string
  updateTime?: string
}

export function login(payload: LoginPayload) {
  return request<WvpResult<LoginResult>>({
    url: '/user/login',
    method: 'get',
    params: {
      username: payload.username.trim(),
      password: md5(payload.password)
    }
  })
}

export function logout() {
  return request<WvpResult>({
    url: '/user/logout',
    method: 'get'
  })
}

export function getUserInfo() {
  return request<WvpResult<UserInfoResult>>({
    method: 'post',
    url: '/user/userInfo'
  })
}

export interface User {
  id?: number
  username: string
  password?: string
  /** 后端返回**嵌套**角色对象（WVP `User.role`），不是扁平的 roleId/roleName */
  role?: { id: number; name: string; authority?: string }
  pushKey?: string
  createTime?: string
  updateTime?: string
}

export interface UserQueryParams {
  page?: number
  count?: number
  query?: string
}

export function getUserList(params: UserQueryParams) {
  return request<WvpResult<{ total: number; list: User[] }>>({
    method: 'get',
    url: '/user/users',
    params
  })
}

/**
 * 新增用户。
 *
 * 口令**不做** md5：WVP 的契约是 `add` 收明文、由服务端补 md5 再入库
 * （`UserController.java:134`）。后端 `password_secret()` 与登录侧
 * （登录送 `md5(明文)`）用的是同一个秘密值，所以这里必须送明文。
 */
export function addUser(data: { username: string; password: string; roleId: number }) {
  return request<WvpResult>({
    method: 'post',
    url: '/user/add',
    params: data
  })
}

export function deleteUser(id: number | string) {
  return request<WvpResult>({
    method: 'delete',
    url: '/user/delete',
    params: { id }
  })
}

/**
 * 自助改密。
 *
 * `oldPassword` 与登录一样送 `md5(明文)`（WVP 前端也是这么做的：
 * `changePassword.vue` 先 md5 再发）；`password` 送明文，由服务端统一
 * `md5` 后再做 Argon2id —— 这样改完密码下次登录（送 md5）能对上。
 */
export function changePassword(data: { oldPassword: string; password: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/user/changePassword',
    params: { oldPassword: md5(data.oldPassword), password: data.password }
  })
}

export function changePasswordForAdmin(data: { userId: number | string; password: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/user/changePasswordForAdmin',
    params: data
  })
}

export function changePushKey(data: { userId: number | string; pushKey: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/user/changePushKey',
    params: data
  })
}

export function getRoleAll() {
  return request<WvpResult<{ id: number; name: string }[]>>({
    method: 'get',
    url: '/role/all'
  })
}
