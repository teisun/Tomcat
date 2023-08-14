package com.tomcat.controller.response;


import lombok.Data;

@Data
public class UserDTO {

    private String id;

    private String username;

    private String email;
    private String phoneNum;

    private String deviceId;
}
