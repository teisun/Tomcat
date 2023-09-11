package com.tomcat.service;

import com.tomcat.controller.requeset.AuthenticationReq;
import com.tomcat.controller.response.AuthenticationResp;
import com.tomcat.controller.response.UserResp;
import com.tomcat.domain.User;

import java.util.List;

public interface UserService {
  AuthenticationResp register(AuthenticationReq request);
  
  AuthenticationResp login(AuthenticationReq request);

  User findByUsername(String username);

  List<UserResp> findByDeviceId(String deviceId);

  AuthenticationResp registerOrLogin(AuthenticationReq request);
}