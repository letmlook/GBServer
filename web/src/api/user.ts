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
  roleId?: number
  roleName?: string
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

export function changePassword(data: { oldPassword: string; password: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/user/changePassword',
    params: data
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
