def csv_success(status, body):
    return status == 200 and body.lstrip("\ufeff").splitlines()[0:1] == [
        "id,time,level,logger,thread,source,message"
    ]


def missing_source_rejected(status, code, message):
    return status == 200 and code == 500 and "ZLM error:" in message and "DESCRIBE:404" in message


def smoke_exit_code(failures):
    return 1 if failures else 0
