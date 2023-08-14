package com.tomcat.service;

import com.tomcat.controller.requeset.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.controller.response.UserDTO;
import com.tomcat.domain.User;

import java.util.List;
import java.util.Optional;

public interface UserService {
  AuthenticationResponse register(AuthenticationRequest request);
  
  AuthenticationResponse login(AuthenticationRequest request);

  Optional<User> findByUsername(String username);

  List<UserDTO> findByDeviceId(String deviceId);

  AuthenticationResponse registerOrLogin(AuthenticationRequest request);
}