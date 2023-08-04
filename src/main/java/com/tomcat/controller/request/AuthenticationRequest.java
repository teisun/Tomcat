package com.tomcat.controller.request;

import lombok.Data;

import javax.persistence.Column;

/**
 * 登录请求参数DTO类
 */
@Data
public class AuthenticationRequest {

    public String username;

    public String password;

    public String email;

    public String phoneNum;

    public String deviceId;
}
