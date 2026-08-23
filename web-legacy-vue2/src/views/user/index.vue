<template>
  <div class="gb-page">
    <div class="gb-page__header">
      <div>
        <h1 class="gb-page__title">用户与权限</h1>
        <p class="gb-page__subtitle">系统账户 · 角色 · 资源授权</p>
      </div>
      <div class="gb-page__actions">
        <button class="gb-tab" :class="{ 'is-active': tab === 'user' }" @click="tab='user'">用户</button>
        <button class="gb-tab" :class="{ 'is-active': tab === 'role' }" @click="tab='role'">角色</button>
        <button class="gb-tab" :class="{ 'is-active': tab === 'policy' }" @click="tab='policy'">资源授权</button>
        <button class="gb-btn">导出</button>
        <button class="gb-btn gb-btn--primary">+ 新增</button>
      </div>
    </div>

    <section v-if="tab === 'user'" class="users">
      <article class="gb-card">
        <header class="gb-card-title">
          <span>用户列表</span>
          <div class="gb-toolbar">
            <div class="gb-search" style="flex:0 1 220px">
              <svg viewBox="0 0 24 24" fill="none"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6"/><path d="M21 21l-4.35-4.35" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
              <input placeholder="搜索用户名 / 邮箱">
            </div>
            <select class="search-mini">
              <option>全部角色</option><option>超级管理员</option><option>运维</option><option>普通用户</option>
            </select>
          </div>
        </header>
        <el-table :data="users" stripe size="small" style="width:100%">
          <el-table-column prop="name" label="用户" min-width="180">
            <template slot-scope="{ row }">
              <div class="user-cell">
                <div class="user-cell__avatar" :style="{ background: row.avatar }">{{ row.name[0] }}</div>
                <div>
                  <div>{{ row.name }}</div>
                  <div class="text-tertiary text-xs">{{ row.email }}</div>
                </div>
              </div>
            </template>
          </el-table-column>
          <el-table-column prop="role" label="角色" width="120">
            <template slot-scope="{ row }"><span :class="['gb-chip', 'gb-chip--' + row.roleTone]">{{ row.role }}</span></template>
          </el-table-column>
          <el-table-column prop="groups" label="所属组织" min-width="160">
            <template slot-scope="{ row }"><span class="text-tertiary">{{ row.groups }}</span></template>
          </el-table-column>
          <el-table-column prop="online" label="在线" width="100">
            <template slot-scope="{ row }">
              <span :class="['gb-dot', 'gb-dot--' + (row.online ? 'success' : 'mute')]" />
              <span style="margin-left:6px" class="text-tertiary">{{ row.online ? '在线' : '离线' }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="lastLogin" label="最近登录" min-width="180">
            <template slot-scope="{ row }"><span class="mono text-tertiary">{{ row.lastLogin }}</span></template>
          </el-table-column>
          <el-table-column prop="state" label="状态" width="100">
            <template slot-scope="{ row }"><el-switch v-model="row.enabled" /></template>
          </el-table-column>
          <el-table-column label="操作" align="right" width="220">
            <template slot-scope="{ row }">
              <button class="gb-btn-link">详情</button>
              <button class="gb-btn-link">改密</button>
              <button class="gb-btn-link">授权</button>
              <button class="gb-btn-link">删除</button>
            </template>
          </el-table-column>
        </el-table>
        <el-pagination layout="prev, pager, next, jumper, total" :total="48" :page-size="20" class="mt-2" />
      </article>
    </section>

    <section v-else-if="tab === 'role'" class="roles">
      <article class="gb-card">
        <header class="gb-card-title"><span>角色列表</span><button class="gb-btn gb-btn--primary">+ 新建角色</button></header>
        <div class="role-grid">
          <div v-for="r in roles" :key="r.name" class="role-card">
            <div class="role-card__head">
              <div>
                <div class="role-card__name">{{ r.name }}</div>
                <div class="text-tertiary text-xs">{{ r.users }} 用户</div>
              </div>
              <span :class="['gb-chip', 'gb-chip--' + r.tone]">{{ r.scope }}</span>
            </div>
            <ul class="role-card__perms">
              <li v-for="p in r.perms" :key="p">{{ p }}</li>
            </ul>
            <footer class="role-card__foot">
              <button class="gb-btn-link">编辑</button>
              <button class="gb-btn-link">复制</button>
              <button class="gb-btn-link">删除</button>
            </footer>
          </div>
        </div>
      </article>
    </section>

    <section v-else class="policy">
      <article class="gb-card">
        <header class="gb-card-title"><span>资源授权矩阵</span><span class="meta">行：用户/角色 · 列：资源</span></header>
        <table class="perm-table">
          <thead>
            <tr>
              <th>角色 / 资源</th>
              <th v-for="c in cols" :key="c">{{ c }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in rows" :key="r.name">
              <td class="r-name">{{ r.name }}</td>
              <td v-for="c in cols" :key="c">
                <el-checkbox v-model="r.m[c]" />
              </td>
            </tr>
          </tbody>
        </table>
      </article>
    </section>
  </div>
</template>

<script>
export default {
  name: 'User',
  data() {
    return {
      tab: 'user',
      cols: ['设备', '通道', '录像', '用户', '日志', '媒体', '平台', '告警'],
      users: [
        { name: 'admin', email: 'admin@gbserver.cn', role: '超级管理员', roleTone: 'error', groups: '总部', online: true, lastLogin: '刚刚 · 127.0.0.1', enabled: true, avatar: 'linear-gradient(135deg,#0b8ab2,#05546f)' },
        { name: 'ops-tianhe', email: 'ops-tianhe@gbserver.cn', role: '运维', roleTone: 'info', groups: '天河区', online: true, lastLogin: '2 分前 · 10.20.4.21', enabled: true, avatar: 'linear-gradient(135deg,#16a34a,#05546f)' },
        { name: 'ops-haizhu', email: 'ops-haizhu@gbserver.cn', role: '运维', roleTone: 'info', groups: '海珠区', online: false, lastLogin: '4 时前 · 10.21.4.10', enabled: true, avatar: 'linear-gradient(135deg,#ea8a0c,#dc2626)' },
        { name: 'viewer01', email: 'viewer01@gbserver.cn', role: '普通用户', roleTone: 'default', groups: '天河区', online: false, lastLogin: '昨天 16:12', enabled: true, avatar: 'linear-gradient(135deg,#5eb4d4,#2a93bd)' },
        { name: 'viewer02', email: 'viewer02@gbserver.cn', role: '普通用户', roleTone: 'default', groups: '番禺区', online: false, lastLogin: '3 天前', enabled: false, avatar: 'linear-gradient(135deg,#9ad0e6,#5eb4d4)' }
      ],
      roles: [
        { name: '超级管理员', scope: '全局', tone: 'error', users: 3, perms: ['设备管理', '用户管理', '角色管理', '日志审计', '系统配置', '策略授权', '录像控制', '告警管理', '平台对接'] },
        { name: '运维', scope: '组织', tone: 'info', users: 8, perms: ['设备管理', '录像控制', '告警管理', '日志审计'] },
        { name: '操作员', scope: '组织', tone: 'success', users: 12, perms: ['录像控制', '告警管理'] },
        { name: '普通用户', scope: '个人', tone: 'default', users: 24, perms: ['查看通道', '回放录像'] },
        { name: '审计', scope: '组织', tone: 'warning', users: 1, perms: ['日志审计'] }
      ],
      rows: [
        { name: '超级管理员', m: { '设备': true, '通道': true, '录像': true, '用户': true, '日志': true, '媒体': true, '平台': true, '告警': true } },
        { name: '运维', m: { '设备': true, '通道': true, '录像': true, '用户': false, '日志': true, '媒体': true, '平台': true, '告警': true } },
        { name: '操作员', m: { '设备': false, '通道': true, '录像': true, '用户': false, '日志': false, '媒体': false, '平台': false, '告警': true } },
        { name: '普通用户', m: { '设备': false, '通道': true, '录像': true, '用户': false, '日志': false, '媒体': false, '平台': false, '告警': false } }
      ]
    }
  }
}
</script>

<style lang="scss" scoped>
.search-mini { padding: 4px 8px; font-size: 11px; border: 1px solid var(--border-default); background: var(--bg-elevated); border-radius: 4px; color: var(--text-primary); outline: 0; }

.user-cell { display: flex; align-items: center; gap: 10px; }
.user-cell__avatar { width: 28px; height: 28px; border-radius: 6px; display: grid; place-items: center; color: #fff; font-weight: 600; font-size: 12px; }

.role-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 12px; }
.role-card { background: var(--bg-base); border: 1px solid var(--border-subtle); border-radius: 8px; padding: 14px; display: flex; flex-direction: column; gap: 12px; }
.role-card__head { display: flex; justify-content: space-between; align-items: flex-start; }
.role-card__name { font-size: var(--text-md); font-weight: 600; }
.role-card__perms { list-style: none; padding: 0; margin: 0; display: flex; flex-wrap: wrap; gap: 6px; }
.role-card__perms li { font-size: 11px; padding: 3px 8px; background: var(--bg-elevated); border-radius: 4px; color: var(--text-secondary); border: 1px solid var(--border-subtle); }
.role-card__foot { display: flex; gap: 12px; border-top: 1px solid var(--border-subtle); padding-top: 10px; }

.perm-table { width: 100%; border-collapse: separate; border-spacing: 0; font-size: 12px; }
.perm-table th { background: var(--bg-elevated); padding: 10px 12px; text-align: left; color: var(--text-tertiary); font-weight: 500; border-bottom: 1px solid var(--border-default); }
.perm-table td { padding: 10px 12px; border-bottom: 1px solid var(--border-subtle); }
.perm-table .r-name { font-weight: 500; }
.mt-2 { margin-top: 8px; }
</style>
