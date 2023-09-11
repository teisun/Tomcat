package com.tomcat.controller.requeset;

import lombok.Data;

/**
 * 登录请求参数DTO类
 */
@Data
public class AuthenticationReq {

    public String username;

    public String password;

    public String email;

    public String phoneNum;

    public String deviceId;

    public AuthenticationReq(String username, String password, String email, String phoneNum, String deviceId) {
        this.username = username;
        this.password = password;
        this.email = email;
        this.phoneNum = phoneNum;
        this.deviceId = deviceId;
    }
}
