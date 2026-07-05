import md5 from 'js-md5'
import { request } from '@/utils/request'

export interface LoginPayload {
  username: string
  password: string
}

export interface LoginResult {
  accessToken: string
  username: string
  serverId: string
}

export function login(params: LoginPayload) {
  return request<WvpResult<LoginResult>>({
    url: '/api/user/login',
    method: 'get',
    params: {
      username: params.username.trim(),
      password: md5(params.password)
    }
  })
}

export function logout() {
  return request<WvpResult>({
    url: '/api/user/logout',
    method: 'get'
  })
}

export function getUserInfo() {
  return request<WvpResult>({
    method: 'post',
    url: '/api/user/userInfo'
  })
}

export function changePushKey(params: { pushKey: string; userId: string | number }) {
  return request<WvpResult>({
    method: 'post',
    url: '/api/user/changePushKey',
    params
  })
}

export function queryList(params: { page: number; count: number }) {
  return request<WvpResult>({
    method: 'get',
    url: '/api/user/users',
    params
  })
}

export function removeById(id: string | number) {
  return request<WvpResult>({
    method: 'delete',
    url: `/api/user/delete?id=${id}`
  })
}

export function add(params: { username: string; password: string; roleId: string | number }) {
  return request<WvpResult>({
    method: 'post',
    url: '/api/user/add',
    params
  })
}

export function changePassword(params: { oldPassword: string; password: string }) {
  return request<WvpResult>({
    method: 'post',
    url: '/api/user/changePassword',
    params
  })
}

export function changePasswordForAdmin(params: { password: string; userId: string | number }) {
  return request<WvpResult>({
    method: 'post',
    url: '/api/user/changePasswordForAdmin',
    params
  })
}
