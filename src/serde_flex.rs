//! 宽容的 serde 反序列化助手：**同一字段既接受 JSON 数字、也接受字符串**。
//!
//! ## 为什么需要它
//!
//! 这个仓库的接口同时服务三类调用方，而它们对"编号类字段"的编码习惯不同：
//!
//! * 本仓库 Vue3 前端：有的地方 `map(String)` 发字符串（如云端录像删除的 `ids`），
//!   有的地方直接发 `el-input` 的字符串（如区域国标编码），
//!   还有的地方发 `el-select` 的数字；
//! * 第三方集成 / 脚本化客户端：按各自 DTO 的类型发（多为整数）；
//! * 手工 curl / 脚本：随手写 `1` 或 `"1"`。
//!
//! 严格的 `Option<i64>` / `Vec<String>` 会让其中一类调用方直接 **422**
//! （`invalid type: integer 43, expected a string` 之类），而这类失败
//! **只在特定调用方上出现**，很容易长期潜伏 —— 2026-09-12 就在
//! `DELETE /api/cloud/record/delete`（前端 `map(String)`，脚本发整数）与
//! `/api/jt1078/terminal/add`（颜色发数字、省域发字符串）上各踩了一次。
//!
//! 因此：**编号/ID 类字段一律用本模块的助手**，把"数字还是字符串"这种
//! 表示差异挡在 DTO 之外。
//!
//! 注意 `deserialize_with` 会**去掉** `Option<T>` 字段的隐式 default，
//! 所以每个使用处都要显式写 `#[serde(default, deserialize_with = "...")]`
//! —— 否则该字段会变成必填（历史上 `/api/platform/add` 就因此必然 422）。

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

/// 任意标量 → 字符串。数字/布尔按字面量转，字符串原样返回，`null`/缺失 → `None`。
#[derive(Deserialize)]
#[serde(untagged)]
enum AnyScalar {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl AnyScalar {
    fn into_string(self) -> String {
        match self {
            AnyScalar::Bool(b) => b.to_string(),
            AnyScalar::Int(i) => i.to_string(),
            AnyScalar::Float(f) => {
                // 整数值的浮点（`0.0`）输出成 `0`，避免出现 `"0.0"` 这类编号
                if f.fract() == 0.0 && f.is_finite() {
                    format!("{}", f as i64)
                } else {
                    f.to_string()
                }
            }
            AnyScalar::Str(s) => s,
        }
    }
}

/// `Option<String>`，接受字符串 / 数字 / 布尔 / `null`。
pub fn de_opt_string<'de, D>(de: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<AnyScalar>::deserialize(de)?.map(AnyScalar::into_string))
}

/// `Option<Vec<String>>`，元素接受字符串 / 数字 / 布尔。
///
/// 典型场景：`DELETE /api/cloud/record/delete` body `{"ids":[43]}`（脚本/第三方集成）
/// 与 `{"ids":["43"]}`（本仓库前端）都要能用。
pub fn de_opt_string_vec<'de, D>(de: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<Vec<AnyScalar>>::deserialize(de)?
        .map(|v| v.into_iter().map(AnyScalar::into_string).collect()))
}

/// `Vec<String>`（字段必填，但不接受 `null`）。
pub fn de_string_vec<'de, D>(de: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Vec::<AnyScalar>::deserialize(de)?
        .into_iter()
        .map(AnyScalar::into_string)
        .collect())
}

/// `Option<i64>`，接受数字 / 数字字符串（**空串按"未提供"**），其它值报错。
pub fn de_opt_i64<'de, D>(de: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<AnyScalar>::deserialize(de)? {
        None => Ok(None),
        Some(AnyScalar::Int(i)) => Ok(Some(i)),
        Some(AnyScalar::Str(s)) => {
            let t = s.trim();
            if t.is_empty() {
                Ok(None)
            } else {
                t.parse::<i64>().map(Some).map_err(D::Error::custom)
            }
        }
        Some(AnyScalar::Float(f)) if f.fract() == 0.0 => Ok(Some(f as i64)),
        Some(other) => Err(D::Error::custom(format!(
            "期望整数（或数字字符串），得到 {:?}",
            other.into_string()
        ))),
    }
}

/// `Option<bool>`，接受布尔 / `"true"|"false"` / `1|0`（空串按"未提供"）。
///
/// 前端把开关值放在查询串里时常常是 `"true"`/`"1"`；只认 `bool` 会 422。
pub fn de_opt_bool<'de, D>(de: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<AnyScalar>::deserialize(de)? {
        None => Ok(None),
        Some(AnyScalar::Bool(b)) => Ok(Some(b)),
        Some(AnyScalar::Int(i)) => Ok(Some(i != 0)),
        Some(AnyScalar::Float(f)) => Ok(Some(f != 0.0)),
        Some(AnyScalar::Str(s)) => {
            let t = s.trim().to_ascii_lowercase();
            match t.as_str() {
                "" => Ok(None),
                "true" | "1" | "on" | "yes" => Ok(Some(true)),
                "false" | "0" | "off" | "no" => Ok(Some(false)),
                other => Err(D::Error::custom(format!(
                    "期望布尔值（true/false/1/0），得到 {:?}",
                    other
                ))),
            }
        }
    }
}

/// `Option<Vec<i64>>`，元素接受数字 / 数字字符串（空串元素被丢弃）。
pub fn de_opt_i64_vec<'de, D>(de: D) -> Result<Option<Vec<i64>>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<Vec<AnyScalar>>::deserialize(de)?;
    let Some(items) = raw else { return Ok(None) };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            AnyScalar::Int(i) => out.push(i),
            AnyScalar::Float(f) if f.fract() == 0.0 => out.push(f as i64),
            AnyScalar::Str(s) => {
                let t = s.trim();
                if t.is_empty() {
                    continue;
                }
                out.push(t.parse::<i64>().map_err(D::Error::custom)?);
            }
            other => {
                return Err(D::Error::custom(format!(
                    "期望整数数组（元素可为数字字符串），得到 {:?}",
                    other.into_string()
                )))
            }
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct OptStr {
        #[serde(default, deserialize_with = "de_opt_string")]
        v: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct OptStrVec {
        #[serde(default, deserialize_with = "de_opt_string_vec")]
        ids: Option<Vec<String>>,
    }

    #[derive(Debug, Deserialize)]
    struct OptI64 {
        #[serde(default, deserialize_with = "de_opt_i64")]
        v: Option<i64>,
    }

    #[derive(Debug, Deserialize)]
    struct OptI64Vec {
        #[serde(default, deserialize_with = "de_opt_i64_vec")]
        ids: Option<Vec<i64>>,
    }

    #[test]
    fn opt_string_accepts_numbers_and_strings() {
        for (json, want) in [
            (serde_json::json!({"v": "abc"}), Some("abc")),
            (serde_json::json!({"v": 12}), Some("12")),
            (serde_json::json!({"v": 0}), Some("0")),
            (serde_json::json!({"v": 0.0}), Some("0")),
            (serde_json::json!({"v": true}), Some("true")),
            (serde_json::json!({"v": null}), None),
            (serde_json::json!({}), None),
        ] {
            let got: OptStr = serde_json::from_value(json.clone()).expect("应可解析");
            assert_eq!(got.v.as_deref(), want, "json={json}");
        }
    }

    /// 回归：`DELETE /api/cloud/record/delete` 的 `{"ids":[43]}`（脚本发整数）
    /// 与 `{"ids":["43"]}`（本仓库前端 `map(String)`）都必须能解析。
    #[test]
    fn string_vec_accepts_mixed_scalars() {
        let a: OptStrVec = serde_json::from_value(serde_json::json!({"ids": [43]})).unwrap();
        assert_eq!(a.ids.unwrap(), vec!["43".to_string()]);

        let b: OptStrVec = serde_json::from_value(serde_json::json!({"ids": ["43", 44]})).unwrap();
        assert_eq!(b.ids.unwrap(), vec!["43".to_string(), "44".to_string()]);

        let c: OptStrVec = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(c.ids, None);
    }

    #[test]
    fn opt_i64_tolerates_empty_and_numeric_strings() {
        let a: OptI64 = serde_json::from_value(serde_json::json!({"v": ""})).unwrap();
        assert_eq!(a.v, None);
        let b: OptI64 = serde_json::from_value(serde_json::json!({"v": "7"})).unwrap();
        assert_eq!(b.v, Some(7));
        let c: OptI64 = serde_json::from_value(serde_json::json!({"v": 7})).unwrap();
        assert_eq!(c.v, Some(7));
        let d: OptI64 = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(d.v, None);
        // 非法值必须报错，不能静默变成"没有筛选条件"
        assert!(serde_json::from_value::<OptI64>(serde_json::json!({"v": "abc"})).is_err());
    }

    #[test]
    fn opt_i64_vec_skips_blanks_and_rejects_garbage() {
        let a: OptI64Vec = serde_json::from_value(serde_json::json!({"ids": [1, "2", ""]})).unwrap();
        assert_eq!(a.ids.unwrap(), vec![1, 2]);
        assert!(serde_json::from_value::<OptI64Vec>(serde_json::json!({"ids": ["x"]})).is_err());
    }
}
