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
///
/// **变体顺序不影响匹配结果**：这几个变体在 JSON 类型上互斥 ——
/// `Str` 只吃 JSON 字符串、`Bool` 只吃 JSON 布尔、`Int`/`U64` 只吃 JSON 整数、
/// `Float` 只吃 JSON 小数与溢出整数。serde 的 untagged 虽然按声明顺序逐个尝试，
/// 但因为互斥，谁先谁后得到的都是同一个结果。
///
/// （历史记录：曾把 `Str` 从末尾提到最前，理由是"字符串会先被 `Int(i64)` 吃掉
/// 并溢出报错"。**该理由经实测不成立** —— serde 的
/// `ContentDeserializer::deserialize_integer` 对 `Content::String` 直接返回
/// `invalid_type`，**不会**把字符串当数字解析。真正的报错来自
/// `de_opt_i64` 自己对字符串做 `parse::<i64>()`，20 位国标号会 `PosOverflow`
/// （"number too large to fit in target type"）。
/// 所以要让端点收下 20 位国标号，要改的是 **DTO 的类型**（收字符串），
/// 而不是调这个枚举的变体顺序。）
///
/// **真正的坑在 `Float` 分支**：超过 `i64::MAX` 的整数在 serde_json 里会退化成
/// f64，而 `f as i64` 是**饱和转换** —— `34020000001320000001` 会静默变成
/// `9223372036854775807`（一个根本不存在的编号），而不是报错。
/// 这是「静默给错值」，比报错危险得多：调用方以为传对了，服务端拿着错编号去
/// 查/去删，查不到还算好，**删错**就是数据事故。故此处改为：
/// `u64` 范围内保精确，超出则**报错要求改用字符串**。
#[derive(Deserialize)]
#[serde(untagged)]
enum AnyScalar {
    Str(String),
    Bool(bool),
    Int(i64),
    U64(u64),
    Float(f64),
}

/// `i64::MAX as f64` 会**向上**取整到 2^63，`u64::MAX as f64` 同样取整到 2^64。
/// 所以上界判断必须用**严格小于**，否则会放行一个饱和后被截断的值。
const I64_LIMIT_F64: f64 = 9223372036854775808.0; // 2^63
const U64_LIMIT_F64: f64 = 18446744073709551616.0; // 2^64
const I64_MIN_F64: f64 = -9223372036854775808.0; // -2^63

/// 编号超出整数可表示范围时的统一文案。
///
/// 超过 `u64::MAX` 的整数在 JSON 里只能以 f64 承载，而 f64 在 2^53 以上
/// **无法精确表示每一个整数**，原始字面量已经丢失，无法还原 —— 只能报错。
const NUMBER_OUT_OF_RANGE: &str =
    "编号超出整数可表示范围（> u64::MAX），请改用字符串形式传递（例如 \"34020000001320000001\")";

impl AnyScalar {
    /// 转成字符串。
    ///
    /// 与旧实现的关键差别：整数值超出可精确表示范围时**报错**，不再静默饱和。
    fn into_string<E: serde::de::Error>(self) -> Result<String, E> {
        match self {
            AnyScalar::Bool(b) => Ok(b.to_string()),
            AnyScalar::Int(i) => Ok(i.to_string()),
            AnyScalar::U64(u) => Ok(u.to_string()),
            AnyScalar::Float(f) => {
                if !f.is_finite() {
                    return Err(E::custom("编号不是有限数值（NaN / 无穷）"));
                }
                if f.fract() != 0.0 {
                    // 真正的小数：保持原有表现（`1.5` → `"1.5"`）。
                    return Ok(f.to_string());
                }
                // 整数值：必须落在可精确表示的范围内，否则报错而不是截断。
                if f < 0.0 {
                    if f < I64_MIN_F64 {
                        return Err(E::custom(NUMBER_OUT_OF_RANGE));
                    }
                    Ok(format!("{}", f as i64))
                } else if f >= U64_LIMIT_F64 {
                    return Err(E::custom(NUMBER_OUT_OF_RANGE));
                } else {
                    Ok(format!("{}", f as u64))
                }
            }
            AnyScalar::Str(s) => Ok(s),
        }
    }
}

/// `Option<String>`，接受字符串 / 数字 / 布尔 / `null`。
pub fn de_opt_string<'de, D>(de: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<AnyScalar>::deserialize(de)? {
        None => Ok(None),
        Some(v) => Ok(Some(v.into_string::<D::Error>()?)),
    }
}

/// `Option<Vec<String>>`，元素接受字符串 / 数字 / 布尔。
///
/// 典型场景：`DELETE /api/cloud/record/delete` body `{"ids":[43]}`（脚本/第三方集成）
/// 与 `{"ids":["43"]}`（本仓库前端）都要能用。
pub fn de_opt_string_vec<'de, D>(de: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<Vec<AnyScalar>>::deserialize(de)? {
        None => Ok(None),
        Some(items) => Ok(Some(
            items
                .into_iter()
                .map(|v| v.into_string::<D::Error>())
                .collect::<Result<Vec<_>, _>>()?,
        )),
    }
}

/// `Vec<String>`（字段必填，但不接受 `null`）。
pub fn de_string_vec<'de, D>(de: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<AnyScalar>::deserialize(de)?
        .into_iter()
        .map(|v| v.into_string::<D::Error>())
        .collect()
}

/// `Option<i64>`，接受数字 / 数字字符串（**空串按"未提供"**），其它值报错。
pub fn de_opt_i64<'de, D>(de: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<AnyScalar>::deserialize(de)? {
        None => Ok(None),
        Some(AnyScalar::Int(i)) => Ok(Some(i)),
        Some(AnyScalar::U64(u)) => u
            .try_into()
            .map(Some)
            .map_err(|_| D::Error::custom("期望整数（或数字字符串），数值超出 i64 范围")),
        Some(AnyScalar::Str(s)) => {
            let t = s.trim();
            if t.is_empty() {
                Ok(None)
            } else {
                t.parse::<i64>().map(Some).map_err(D::Error::custom)
            }
        }
        // 整数值浮点：同样要卡住范围，否则 `f as i64` 会饱和成一个假的编号。
        Some(AnyScalar::Float(f)) if f.fract() == 0.0 && f >= I64_MIN_F64 && f < I64_LIMIT_F64 => {
            Ok(Some(f as i64))
        }
        Some(other) => Err(D::Error::custom(format!(
            "期望整数（或数字字符串），得到 {:?}",
            other.into_string::<D::Error>()?
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
        Some(AnyScalar::U64(u)) => Ok(Some(u != 0)),
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
            AnyScalar::U64(u) => {
                let v: i64 = u
                    .try_into()
                    .map_err(|_| D::Error::custom("期望整数数组，数值超出 i64 范围"))?;
                out.push(v);
            }
            AnyScalar::Float(f) if f.fract() == 0.0 && f >= I64_MIN_F64 && f < I64_LIMIT_F64 => {
                out.push(f as i64)
            }
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
                    other.into_string::<D::Error>()?
                )))
            }
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    /// 20 位国标号（数字串）必须原样返回，不能按 i64 解析后溢出。
    #[test]
    fn big_numeric_string_is_returned_verbatim() {
        #[derive(Deserialize)]
        struct Dto {
            #[serde(default, deserialize_with = "super::de_opt_string")]
            id: Option<String>,
        }
        // 走 axum 的 Query（就是查询串反序列化真正用的那条路）
        let dto: axum::extract::Query<Dto> = axum::extract::Query::try_from_uri(
            &axum::http::Uri::from_static("http://x/?id=34020000001310000001"),
        )
        .unwrap();
        assert_eq!(dto.id.as_deref(), Some("34020000001310000001"));
        // 普通数字串仍原样返回
        let dto: axum::extract::Query<Dto> =
            axum::extract::Query::try_from_uri(&axum::http::Uri::from_static("http://x/?id=123"))
                .unwrap();
        assert_eq!(dto.id.as_deref(), Some("123"));
        // JSON 数字也能收
        let dto: Dto = serde_json::from_str(r#"{"id":43}"#).unwrap();
        assert_eq!(dto.id.as_deref(), Some("43"));
    }

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
        let a: OptI64Vec =
            serde_json::from_value(serde_json::json!({"ids": [1, "2", ""]})).unwrap();
        assert_eq!(a.ids.unwrap(), vec![1, 2]);
        assert!(serde_json::from_value::<OptI64Vec>(serde_json::json!({"ids": ["x"]})).is_err());
    }

    /// 回归：**超过 `i64::MAX` 的整数不能被静默截断**。
    ///
    /// 成因：serde_json 把超出 i64 的 JSON 整数退化成 f64，而 `f as i64` 是
    /// **饱和转换** —— `34020000001320000001` 会变成 `9223372036854775807`
    ///（一个根本不存在的编号）。调用方以为传对了，服务端却拿着错编号去查/去删。
    ///
    /// 现在的行为：
    /// - `u64` 范围内（`≤ u64::MAX`）→ **精确**返回；
    /// - 超出 `u64::MAX` → **报错**，要求改用字符串（f64 已丢失原始字面量，
    ///   无法还原，只能让调用方改口径）。
    #[test]
    fn big_integer_is_exact_or_errors_never_silently_truncated() {
        // u64::MAX：serde_json 精确保留 → 原样返回
        let a: OptStr = serde_json::from_str(r#"{"v":18446744073709551615}"#).unwrap();
        assert_eq!(a.v.as_deref(), Some("18446744073709551615"));

        // i64::MAX + 1：过去会变成 i64::MAX，现在必须精确
        let b: OptStr = serde_json::from_str(r#"{"v":9223372036854775808}"#).unwrap();
        assert_eq!(b.v.as_deref(), Some("9223372036854775808"));

        // 20 位国标号作为 JSON **数字**：超出 u64::MAX，必须报错而不是给错值
        let c = serde_json::from_str::<OptStr>(r#"{"v":34020000001320000001}"#);
        assert!(
            c.is_err(),
            "20 位国标号作为数字必须报错，不能静默变成 i64::MAX"
        );

        // 同一个值作为**字符串**必须照收（这才是正确用法）
        let d: OptStr = serde_json::from_str(r#"{"v":"34020000001320000001"}"#).unwrap();
        assert_eq!(d.v.as_deref(), Some("34020000001320000001"));
    }

    /// 回归：`de_opt_i64` 也不能把超范围的数静默饱和成 `i64::MAX`。
    #[test]
    fn opt_i64_rejects_out_of_range_instead_of_saturating() {
        // 过去：`f as i64` 饱和 → Some(9223372036854775807)
        let a = serde_json::from_str::<OptI64>(r#"{"v":34020000001320000001}"#);
        assert!(a.is_err(), "超范围整数必须报错，不能饱和成 i64::MAX");

        // u64 范围内但超出 i64：同样要报错
        let b = serde_json::from_str::<OptI64>(r#"{"v":18446744073709551615}"#);
        assert!(b.is_err(), "超出 i64 范围的 u64 必须报错");

        // 正常值不受影响
        let c: OptI64 = serde_json::from_str(r#"{"v":9223372036854775807}"#).unwrap();
        assert_eq!(c.v, Some(i64::MAX));
    }

    /// 回归：负数与小数不能被新分支改坏。
    #[test]
    fn negative_and_fractional_values_keep_their_meaning() {
        let a: OptStr = serde_json::from_str(r#"{"v":-5}"#).unwrap();
        assert_eq!(a.v.as_deref(), Some("-5"));
        let b: OptStr = serde_json::from_str(r#"{"v":-5.0}"#).unwrap();
        assert_eq!(b.v.as_deref(), Some("-5"));
        let c: OptStr = serde_json::from_str(r#"{"v":1.5}"#).unwrap();
        assert_eq!(c.v.as_deref(), Some("1.5"));
        let d: OptStr = serde_json::from_str(r#"{"v":0.0}"#).unwrap();
        assert_eq!(d.v.as_deref(), Some("0"));
    }

    /// 回归：字符串数组里的超范围数字同样不能被静默截断
    /// （`DELETE /api/cloud/record/delete` 的 `ids` 走的就是这条路径）。
    #[test]
    fn string_vec_rejects_out_of_range_numbers() {
        let a = serde_json::from_str::<OptStrVec>(r#"{"ids":[34020000001320000001]}"#);
        assert!(a.is_err(), "超范围数字必须报错，不能变成一个假编号");
        // 正确用法：加引号
        let c: OptStrVec = serde_json::from_str(r#"{"ids":["34020000001320000001"]}"#).unwrap();
        assert_eq!(c.ids.unwrap(), vec!["34020000001320000001".to_string()]);
    }
}
