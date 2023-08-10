package com.tomcat.service;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.domain.User;

import java.util.List;
import java.util.Optional;

public interface UserService {
  AuthenticationResponse register(AuthenticationRequest request);
  
  AuthenticationResponse login(AuthenticationRequest request);

  Optional<User> findByUsername(String username);

  List<User> findByDeviceId(String deviceId);

  AuthenticationResponse registerOrLogin(AuthenticationRequest request);
}